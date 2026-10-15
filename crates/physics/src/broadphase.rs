//! Spatial-hash broadphase.
//!
//! Uniform-grid hashing reduces narrowphase candidate pairs from O(n²) to
//! roughly O(n) by bucketing bodies into cells of a fixed world size and only
//! testing bodies that share a cell (or neighbour cells for swept shapes).

use std::collections::HashMap;
use ow_math::Vec3;

/// A spatial hash with a fixed cell size and 3D wrapping keys.
#[derive(Debug, Clone)]
pub struct SpatialHash {
    cell_size: f32,
    inverse_cell_size: f32,
    cells: HashMap<(i32, i32, i32), Vec<u32>>,
    body_cells: HashMap<u32, Vec<(i32, i32, i32)>>,
    stats: QueryStats,
}

/// Counters describing how much work the last query performed, used to measure
/// broadphase effectiveness.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct QueryStats {
    pub queries: u32,
    pub candidates_checked: u32,
    pub pairs_found: u32,
}

/// An axis-aligned bounding box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    /// Builds an AABB, normalising the inputs so `min` really is the minimum.
    pub fn new(a: Vec3, b: Vec3) -> Self {
        Self {
            min: Vec3::new(a.x.min(b.x), a.y.min(b.y), a.z.min(b.z)),
            max: Vec3::new(a.x.max(b.x), a.y.max(b.y), a.z.max(b.z)),
        }
    }

    pub fn from_center_half(center: Vec3, half: Vec3) -> Self {
        Self {
            min: center - half,
            max: center + half,
        }
    }

    pub fn contains(&self, p: Vec3) -> bool {
        p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
            && p.z >= self.min.z
            && p.z <= self.max.z
    }

    pub fn overlaps(&self, other: &Self) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn half_extents(&self) -> Vec3 {
        (self.max - self.min) * 0.5
    }
}

impl SpatialHash {
    pub fn new(cell_size: f32) -> Self {
        let cell_size = if cell_size > 0.0 { cell_size } else { 1.0 };
        Self {
            cell_size,
            inverse_cell_size: 1.0 / cell_size,
            cells: HashMap::new(),
            body_cells: HashMap::new(),
            stats: QueryStats::default(),
        }
    }

    pub fn cell_size(&self) -> f32 {
        self.cell_size
    }

    pub fn body_count(&self) -> usize {
        self.body_cells.len()
    }

    pub fn occupied_cell_count(&self) -> usize {
        self.cells.len()
    }

    /// Maps a world position to its integer cell coordinate.
    fn cell_of(&self, p: Vec3) -> (i32, i32, i32) {
        (
            (p.x * self.inverse_cell_size).floor() as i32,
            (p.y * self.inverse_cell_size).floor() as i32,
            (p.z * self.inverse_cell_size).floor() as i32,
        )
    }

    /// Inserts or replaces the AABB for `body_id`.
    pub fn insert(&mut self, body_id: u32, bounds: Aabb) {
        if let Some(old) = self.body_cells.remove(&body_id) {
            for c in old {
                if let Some(list) = self.cells.get_mut(&c) {
                    list.retain(|&id| id != body_id);
                    if list.is_empty() {
                        self.cells.remove(&c);
                    }
                }
            }
        }

        let min = self.cell_of(bounds.min);
        let max = self.cell_of(bounds.max);

        let mut cells = Vec::new();
        for x in min.0..=max.0 {
            for y in min.1..=max.1 {
                for z in min.2..=max.2 {
                    let c = (x, y, z);
                    self.cells.entry(c).or_default().push(body_id);
                    cells.push(c);
                }
            }
        }
        self.body_cells.insert(body_id, cells);
    }

    pub fn remove(&mut self, body_id: u32) {
        if let Some(old) = self.body_cells.remove(&body_id) {
            for c in old {
                if let Some(list) = self.cells.get_mut(&c) {
                    list.retain(|&id| id != body_id);
                    if list.is_empty() {
                        self.cells.remove(&c);
                    }
                }
            }
        }
    }

    pub fn clear(&mut self) {
        self.cells.clear();
        self.body_cells.clear();
        self.stats = QueryStats::default();
    }

    /// Returns every body whose stored AABB is in the queried cell or a
    /// neighbouring cell, deduplicated.
    pub fn query(&self, center: Vec3) -> Vec<u32> {
        let c = self.cell_of(center);
        let mut found: Vec<u32> = Vec::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let key = (c.0 + dx, c.1 + dy, c.2 + dz);
                    if let Some(list) = self.cells.get(&key) {
                        for id in list {
                            if !found.contains(id) {
                                found.push(*id);
                            }
                        }
                    }
                }
            }
        }
        found
    }

    /// Enumerates every candidate pair of overlapping bodies, each pair once,
    /// with the smaller id first. Pairs whose AABBs no longer overlap are
    /// filtered here so the caller only sees genuine candidates.
    ///
    /// Uses a `HashSet` for de-duplication: with tens of thousands of bodies a
    /// linear scan over the growing pair list is itself quadratic.
    pub fn candidate_pairs(&mut self, aabbs: &HashMap<u32, Aabb>) -> Vec<(u32, u32)> {
        use std::collections::HashSet;

        let mut seen: HashSet<(u32, u32)> = HashSet::new();
        let mut pairs: Vec<(u32, u32)> = Vec::new();
        let mut checked = 0u32;

        let keys: Vec<(i32, i32, i32)> = self.cells.keys().copied().collect();
        for key in keys {
            let list = match self.cells.get(&key) {
                Some(l) if l.len() > 1 => l,
                _ => continue,
            };

            for i in 0..list.len() {
                for j in (i + 1)..list.len() {
                    let (a, b) = (list[i], list[j]);
                    if a == b {
                        continue;
                    }
                    let (lo, hi) = if a < b { (a, b) } else { (b, a) };

                    checked += 1;
                    if !seen.insert((lo, hi)) {
                        continue;
                    }

                    // Confirm the AABBs actually overlap before reporting.
                    let overlap = aabbs
                        .get(&lo)
                        .zip(aabbs.get(&hi))
                        .map(|(ba, bb)| ba.overlaps(bb))
                        .unwrap_or(false);
                    if overlap {
                        pairs.push((lo, hi));
                    }
                }
            }
        }

        self.stats.queries += 1;
        self.stats.candidates_checked += checked;
        self.stats.pairs_found += pairs.len() as u32;
        pairs
    }

    pub fn stats(&self) -> QueryStats {
        self.stats
    }
}

impl Default for SpatialHash {
    fn default() -> Self {
        Self::new(10.0)
    }
}

/// Brute-force O(n²) reference implementation, used by tests to prove the
/// spatial hash returns the same pairs.
pub fn brute_force_pairs(aabbs: &HashMap<u32, Aabb>) -> Vec<(u32, u32)> {
    let mut ids: Vec<u32> = aabbs.keys().copied().collect();
    ids.sort_unstable();

    let mut pairs = Vec::new();
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            let a = ids[i];
            let b = ids[j];
            if aabbs[&a].overlaps(&aabbs[&b]) {
                pairs.push((a, b));
            }
        }
    }
    pairs
}
