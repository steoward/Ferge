use std::collections::{HashMap, HashSet, VecDeque};
use ow_math::Vec3;
use ow_physics::AABB;

/// Unique coordinate for a spatial world sector on the XZ ground plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SectorId {
    pub x: i32,
    pub z: i32,
}

impl SectorId {
    pub fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    /// Converts world coordinates to a sector ID given the grid sector size.
    pub fn from_world_pos(pos: Vec3, sector_size: f32) -> Self {
        let x = (pos.x / sector_size).floor() as i32;
        let z = (pos.z / sector_size).floor() as i32;
        Self { x, z }
    }

    /// Computes the center position of this sector in world coordinates.
    pub fn to_world_center(&self, sector_size: f32) -> Vec3 {
        Vec3::new(
            (self.x as f32 + 0.5) * sector_size,
            0.0,
            (self.z as f32 + 0.5) * sector_size,
        )
    }

    /// Computes the bounding box of this sector.
    pub fn to_bounds(&self, sector_size: f32, min_y: f32, max_y: f32) -> AABB {
        let min_x = self.x as f32 * sector_size;
        let min_z = self.z as f32 * sector_size;
        AABB::new(
            Vec3::new(min_x, min_y, min_z),
            Vec3::new(min_x + sector_size, max_y, min_z + sector_size),
        )
    }
}

/// Level of Detail for streamed world chunks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChunkLod {
    /// Highest geometry detail, full physics collision, dense props and entities.
    High = 0,
    /// Medium geometry detail, simplified collision, key structures only.
    Medium = 1,
    /// Low geometry detail (distant SLOD), no collision bodies.
    Low = 2,
    /// Distant horizon billboard / impostor representation.
    Impostor = 3,
}

/// Lifecycle state of a streamed world chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChunkState {
    Unloaded,
    Queued,
    Loading,
    Loaded,
    Active,
    Unloading,
}

/// Represents a partitioned chunk of the open world.
#[derive(Debug, Clone)]
pub struct WorldChunk {
    pub id: SectorId,
    pub bounds: AABB,
    pub current_lod: ChunkLod,
    pub target_lod: ChunkLod,
    pub state: ChunkState,
    pub memory_size_bytes: usize,
    pub last_accessed_frame: u64,
    pub entities: Vec<u64>,
    pub mesh_ids: Vec<u32>,
    pub collision_body_ids: Vec<u32>,
}

impl WorldChunk {
    pub fn new(id: SectorId, sector_size: f32, min_y: f32, max_y: f32, estimated_memory: usize) -> Self {
        let bounds = id.to_bounds(sector_size, min_y, max_y);
        Self {
            id,
            bounds,
            current_lod: ChunkLod::Impostor,
            target_lod: ChunkLod::Impostor,
            state: ChunkState::Unloaded,
            memory_size_bytes: estimated_memory,
            last_accessed_frame: 0,
            entities: Vec::new(),
            mesh_ids: Vec::new(),
            collision_body_ids: Vec::new(),
        }
    }

    pub fn is_resident(&self) -> bool {
        matches!(self.state, ChunkState::Loaded | ChunkState::Active)
    }
}

/// Memory and concurrency limits for the streaming subsystem.
#[derive(Debug, Clone)]
pub struct StreamingBudget {
    /// Maximum allowed memory footprint for loaded chunks in bytes.
    pub max_memory_bytes: usize,
    /// Current memory used by resident chunks in bytes.
    pub current_memory_bytes: usize,
    /// Maximum number of chunks allowed to transition into loading state per frame.
    pub max_loading_per_frame: usize,
}

impl Default for StreamingBudget {
    fn default() -> Self {
        Self {
            max_memory_bytes: 512 * 1024 * 1024, // 512 MB default budget
            current_memory_bytes: 0,
            max_loading_per_frame: 4,
        }
    }
}

/// Configuration parameters for distance tiers and predictive streaming.
#[derive(Debug, Clone)]
pub struct StreamingConfig {
    pub sector_size: f32,
    pub high_lod_radius: f32,
    pub medium_lod_radius: f32,
    pub low_lod_radius: f32,
    pub max_streaming_radius: f32,
    pub velocity_anticipation_seconds: f32,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            sector_size: 200.0,
            high_lod_radius: 250.0,
            medium_lod_radius: 500.0,
            low_lod_radius: 1000.0,
            max_streaming_radius: 1400.0,
            velocity_anticipation_seconds: 3.0,
        }
    }
}

/// Open world streaming manager inspired by RAGE's spatial grid and predictive cache.
pub struct StreamingManager {
    pub config: StreamingConfig,
    pub budget: StreamingBudget,
    chunks: HashMap<SectorId, WorldChunk>,
    active_sectors: HashSet<SectorId>,
    load_queue: VecDeque<SectorId>,
    current_frame: u64,
}

impl StreamingManager {
    pub fn new(config: StreamingConfig, budget: StreamingBudget) -> Self {
        Self {
            config,
            budget,
            chunks: HashMap::new(),
            active_sectors: HashSet::new(),
            load_queue: VecDeque::new(),
            current_frame: 0,
        }
    }

    /// Registers a chunk definition into the world grid.
    pub fn register_chunk(&mut self, chunk: WorldChunk) {
        self.chunks.insert(chunk.id, chunk);
    }

    pub fn get_chunk(&self, id: SectorId) -> Option<&WorldChunk> {
        self.chunks.get(&id)
    }

    pub fn get_chunk_mut(&mut self, id: SectorId) -> Option<&mut WorldChunk> {
        self.chunks.get_mut(&id)
    }

    pub fn total_chunks(&self) -> usize {
        self.chunks.len()
    }

    pub fn active_chunk_count(&self) -> usize {
        self.active_sectors.len()
    }

    /// Anticipates future player position based on current velocity (RAGE predictive streaming).
    pub fn predict_focus_point(&self, camera_pos: Vec3, velocity: Vec3) -> Vec3 {
        camera_pos + velocity * self.config.velocity_anticipation_seconds
    }

    /// Determines the desired LOD tier for a given distance from the focus point.
    pub fn determine_lod(&self, distance: f32) -> Option<ChunkLod> {
        if distance <= self.config.high_lod_radius {
            Some(ChunkLod::High)
        } else if distance <= self.config.medium_lod_radius {
            Some(ChunkLod::Medium)
        } else if distance <= self.config.low_lod_radius {
            Some(ChunkLod::Low)
        } else if distance <= self.config.max_streaming_radius {
            Some(ChunkLod::Impostor)
        } else {
            None
        }
    }

    /// Updates the streaming state machine based on the camera position and velocity.
    pub fn update(&mut self, camera_pos: Vec3, velocity: Vec3, frame: u64) {
        self.current_frame = frame;
        let focus_point = self.predict_focus_point(camera_pos, velocity);

        let mut desired_sectors: HashMap<SectorId, (ChunkLod, f32)> = HashMap::new();

        // Evaluate all registered chunks
        for (&id, chunk) in &self.chunks {
            let center = chunk.bounds.center();
            // Distance to current camera and to predicted focus point
            let dist_camera = (center - camera_pos).length();
            let dist_focus = (center - focus_point).length();
            let effective_dist = dist_camera.min(dist_focus);

            if let Some(target_lod) = self.determine_lod(effective_dist) {
                desired_sectors.insert(id, (target_lod, effective_dist));
            }
        }

        // Process newly requested or LOD-changed chunks
        let mut sorted_requests: Vec<(SectorId, ChunkLod, f32)> = desired_sectors
            .iter()
            .map(|(&id, &(lod, dist))| (id, lod, dist))
            .collect();

        // Sort by distance ascending so closer chunks load first
        sorted_requests.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

        for (id, target_lod, _) in sorted_requests {
            if let Some(chunk) = self.chunks.get_mut(&id) {
                chunk.target_lod = target_lod;
                chunk.last_accessed_frame = frame;

                match chunk.state {
                    ChunkState::Unloaded => {
                        chunk.state = ChunkState::Queued;
                        self.load_queue.push_back(id);
                    }
                    ChunkState::Loaded => {
                        chunk.state = ChunkState::Active;
                        chunk.current_lod = target_lod;
                        self.active_sectors.insert(id);
                    }
                    ChunkState::Active => {
                        chunk.current_lod = target_lod;
                    }
                    _ => {}
                }
            }
        }

        // Identify out-of-range active chunks to unload
        let mut to_unload = Vec::new();
        for &active_id in &self.active_sectors {
            if !desired_sectors.contains_key(&active_id) {
                to_unload.push(active_id);
            }
        }

        for id in to_unload {
            self.unload_chunk(id);
        }

        // Process a bounded batch of queued loads
        self.process_load_queue();

        // Evict if memory budget is exceeded
        self.enforce_memory_budget();
    }

    /// Dispatches chunks from the queue into loading state up to the per-frame limit.
    fn process_load_queue(&mut self) {
        let mut loaded_this_frame = 0;

        while loaded_this_frame < self.budget.max_loading_per_frame {
            if let Some(id) = self.load_queue.pop_front() {
                if let Some(chunk) = self.chunks.get_mut(&id) {
                    if chunk.state == ChunkState::Queued {
                        chunk.state = ChunkState::Loaded;
                        chunk.current_lod = chunk.target_lod;
                        self.budget.current_memory_bytes += chunk.memory_size_bytes;
                        self.active_sectors.insert(id);
                        loaded_this_frame += 1;
                    }
                }
            } else {
                break;
            }
        }
    }

    /// Unloads an individual chunk, releasing its resident memory.
    pub fn unload_chunk(&mut self, id: SectorId) {
        if let Some(chunk) = self.chunks.get_mut(&id) {
            if chunk.is_resident() {
                self.budget.current_memory_bytes = self
                    .budget
                    .current_memory_bytes
                    .saturating_sub(chunk.memory_size_bytes);
            }
            chunk.state = ChunkState::Unloaded;
            chunk.current_lod = ChunkLod::Impostor;
            self.active_sectors.remove(&id);
        }
    }

    /// Enforces the memory budget by evicting least-recently-accessed chunks furthest from focus.
    fn enforce_memory_budget(&mut self) {
        if self.budget.current_memory_bytes <= self.budget.max_memory_bytes {
            return;
        }

        // Gather resident candidates sorted by oldest access frame
        let mut candidates: Vec<(SectorId, u64)> = self
            .active_sectors
            .iter()
            .filter_map(|&id| {
                self.chunks
                    .get(&id)
                    .map(|c| (id, c.last_accessed_frame))
            })
            .collect();

        candidates.sort_by_key(|&(_, frame)| frame);

        for (id, _) in candidates {
            if self.budget.current_memory_bytes <= self.budget.max_memory_bytes {
                break;
            }
            self.unload_chunk(id);
        }
    }
}
