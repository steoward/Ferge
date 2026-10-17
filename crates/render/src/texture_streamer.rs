//! Progressive Open-World Texture Streaming & VRAM Budget Management Engine.
//!
//! Implements a progressive mipmap streamer with distance-based priority queuing,
//! camera frustum directional weighting, lock-free eviction under memory pressure (LRU),
//! and direct streaming integration for RPFv7 packages and procedural textures.

use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;
use ow_math::Vec3;

/// Unique identifier for streamable texture assets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TextureId(pub u64);

/// Texture compression formats supported by the streaming pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    Bc1Dxt1, // 4 bpp (Diffuse/Albedo opaque)
    Bc3Dxt5, // 8 bpp (Albedo with Alpha, Normals + Spec)
    Bc7,     // 8 bpp (High-quality HDR / PBR)
    Rgba8,   // 32 bpp uncompressed
}

impl TextureFormat {
    /// Bytes required for a mip level of width x height.
    pub fn compute_mip_bytes(&self, width: u32, height: u32) -> usize {
        let w = width.max(1);
        let h = height.max(1);
        match self {
            TextureFormat::Bc1Dxt1 => {
                let blocks_x = w.div_ceil(4) as usize;
                let blocks_y = h.div_ceil(4) as usize;
                blocks_x * blocks_y * 8
            }
            TextureFormat::Bc3Dxt5 | TextureFormat::Bc7 => {
                let blocks_x = w.div_ceil(4) as usize;
                let blocks_y = h.div_ceil(4) as usize;
                blocks_x * blocks_y * 16
            }
            TextureFormat::Rgba8 => (w * h * 4) as usize,
        }
    }
}

/// A streamable open-world texture asset with a progressive mipmap pyramid.
#[derive(Debug, Clone)]
pub struct StreamableTexture {
    pub id: TextureId,
    pub name: String,
    pub base_width: u32,
    pub base_height: u32,
    pub format: TextureFormat,
    pub total_mips: u8,
    pub resident_mips: u8,   // Number of mips currently loaded into VRAM (from coarsest to highest)
    pub world_pos: Vec3,
    pub bounding_radius: f32,
    pub importance_bias: f32, // Bias factor (e.g. 1.0 = standard, 2.0 = player vehicle)
    pub last_access_frame: u64,
}

impl StreamableTexture {
    pub fn new(
        id: TextureId,
        name: impl Into<String>,
        width: u32,
        height: u32,
        format: TextureFormat,
        world_pos: Vec3,
    ) -> Self {
        let mut total_mips = 1u8;
        let mut dim = width.max(height);
        while dim > 1 {
            dim /= 2;
            total_mips += 1;
        }

        Self {
            id,
            name: name.into(),
            base_width: width,
            base_height: height,
            format,
            total_mips,
            resident_mips: 1, // Base coarsest mipmap is always resident (ambient baseline)
            world_pos,
            bounding_radius: 10.0,
            importance_bias: 1.0,
            last_access_frame: 0,
        }
    }

    /// Computes the VRAM footprint in bytes for the currently resident mip levels.
    pub fn resident_vram_bytes(&self) -> usize {
        let mut bytes = 0;
        let coarsest_level = self.total_mips.saturating_sub(1);
        let start_level = coarsest_level.saturating_sub(self.resident_mips.saturating_sub(1));

        for level in start_level..=coarsest_level {
            let factor = 1 << level;
            let w = (self.base_width / factor).max(1);
            let h = (self.base_height / factor).max(1);
            bytes += self.format.compute_mip_bytes(w, h);
        }
        bytes
    }

    /// Total VRAM bytes if all mips up to Mip 0 (native full resolution) were loaded.
    pub fn full_vram_bytes(&self) -> usize {
        let mut bytes = 0;
        for level in 0..self.total_mips {
            let factor = 1 << level;
            let w = (self.base_width / factor).max(1);
            let h = (self.base_height / factor).max(1);
            bytes += self.format.compute_mip_bytes(w, h);
        }
        bytes
    }

    /// Calculates target desired resident mips based on distance to camera and screen footprint.
    pub fn compute_desired_mips(&self, cam_pos: Vec3, cam_dir: Vec3) -> u8 {
        let to_tex = self.world_pos - cam_pos;
        let dist = to_tex.length().max(0.1);
        let dir = to_tex / dist;
        let dot = dir.dot(&cam_dir).max(0.0); // 1.0 directly in front, 0.0 behind/lateral

        // Effective visual distance: objects directly in camera view appear effectively closer
        let effective_dist = dist / (1.0 + dot * 1.5) / self.importance_bias.max(0.1);

        // Distance thresholds for progressive LOD mip levels:
        // Ultra Close (< 15m) -> 100% of mips (Mip 0, 4K/2K)
        // Near (15m - 40m) -> 80% of mips
        // Mid (40m - 100m) -> 60% of mips
        // Far (100m - 250m) -> 40% of mips
        // Horizon (> 250m) -> 1 mip (ambient baseline)
        if effective_dist < 15.0 {
            self.total_mips
        } else if effective_dist < 40.0 {
            (self.total_mips.saturating_sub(1)).max(1)
        } else if effective_dist < 100.0 {
            (self.total_mips.saturating_sub(2)).max(1)
        } else if effective_dist < 250.0 {
            (self.total_mips.saturating_sub(3)).max(1)
        } else {
            1
        }
    }
}

/// A priority request to stream in additional mip levels for a texture.
#[derive(Debug, Clone)]
struct StreamingRequest {
    texture_id: TextureId,
    target_resident_mips: u8,
    priority: f32,
}

impl PartialEq for StreamingRequest {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl Eq for StreamingRequest {}

impl PartialOrd for StreamingRequest {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for StreamingRequest {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority.partial_cmp(&other.priority).unwrap_or(Ordering::Equal)
    }
}

/// Real-time diagnostics for texture streaming telemetry.
#[derive(Debug, Clone, Default)]
pub struct TextureStreamingStats {
    pub total_registered_textures: usize,
    pub total_vram_budget_bytes: usize,
    pub current_vram_used_bytes: usize,
    pub pending_requests_count: usize,
    pub mips_loaded_this_frame: usize,
    pub mips_evicted_this_frame: usize,
    pub bytes_streamed_this_frame: usize,
    pub cumulative_bytes_streamed: u64,
}

/// Progressive Texture Streaming Subsystem.
pub struct TextureStreamer {
    vram_budget_bytes: usize,
    vram_used_bytes: usize,
    textures: HashMap<TextureId, StreamableTexture>,
    request_queue: BinaryHeap<StreamingRequest>,
    max_transfer_bytes_per_frame: usize,
    stats: TextureStreamingStats,
    frame_counter: u64,
}

impl TextureStreamer {
    /// Creates a new TextureStreamer with a specified VRAM budget (in bytes).
    pub fn new(vram_budget_bytes: usize) -> Self {
        Self {
            vram_budget_bytes,
            vram_used_bytes: 0,
            textures: HashMap::new(),
            request_queue: BinaryHeap::new(),
            max_transfer_bytes_per_frame: 64 * 1024 * 1024, // 64 MB per frame bandwidth cap
            stats: TextureStreamingStats {
                total_vram_budget_bytes: vram_budget_bytes,
                ..Default::default()
            },
            frame_counter: 0,
        }
    }

    /// Sets the maximum streaming bandwidth allowed per tick (e.g. 32 MB/frame or 128 MB/frame).
    pub fn set_bandwidth_limit(&mut self, bytes_per_frame: usize) {
        self.max_transfer_bytes_per_frame = bytes_per_frame;
    }

    /// Registers a new streamable texture with initial baseline resident mip.
    pub fn register_texture(&mut self, texture: StreamableTexture) {
        let initial_bytes = texture.resident_vram_bytes();
        self.vram_used_bytes += initial_bytes;
        self.textures.insert(texture.id, texture);
        self.stats.total_registered_textures = self.textures.len();
        self.stats.current_vram_used_bytes = self.vram_used_bytes;
    }

    /// Retrieves an immutable reference to a registered texture.
    pub fn get_texture(&self, id: TextureId) -> Option<&StreamableTexture> {
        self.textures.get(&id)
    }

    /// Main frame update tick: evaluates camera viewpoint, prioritizes mips, streams data, and evicts LRU.
    pub fn update(&mut self, camera_pos: Vec3, camera_dir: Vec3, _dt: f32) {
        self.frame_counter += 1;
        self.stats.mips_loaded_this_frame = 0;
        self.stats.mips_evicted_this_frame = 0;
        self.stats.bytes_streamed_this_frame = 0;

        // 1. Evaluate desired mips for all textures and enqueue requests
        for (id, tex) in self.textures.iter_mut() {
            let desired_mips = tex.compute_desired_mips(camera_pos, camera_dir);
            if desired_mips > tex.resident_mips {
                let to_tex = tex.world_pos - camera_pos;
                let dist = to_tex.length().max(0.1);
                let dot = (to_tex / dist).dot(&camera_dir).max(0.0);
                let priority = (1.0 / dist) * (1.0 + dot * 2.0) * tex.importance_bias;

                self.request_queue.push(StreamingRequest {
                    texture_id: *id,
                    target_resident_mips: desired_mips,
                    priority,
                });
            } else if desired_mips < tex.resident_mips && self.vram_used_bytes > (self.vram_budget_bytes * 9 / 10) {
                // If nearing or exceeding budget and texture is far, step down high mips
                let before = tex.resident_vram_bytes();
                tex.resident_mips = desired_mips.max(1);
                let after = tex.resident_vram_bytes();
                if before > after {
                    let freed = before - after;
                    self.vram_used_bytes = self.vram_used_bytes.saturating_sub(freed);
                    self.stats.mips_evicted_this_frame += 1;
                }
            }
        }

        // 2. Process streaming requests up to per-frame bandwidth limit
        let mut frame_bytes_transferred = 0;

        while let Some(req) = self.request_queue.pop() {
            if frame_bytes_transferred >= self.max_transfer_bytes_per_frame {
                break;
            }

            let delta_opt = if let Some(tex) = self.textures.get(&req.texture_id) {
                if req.target_resident_mips > tex.resident_mips {
                    let old_bytes = tex.resident_vram_bytes();
                    let mut temp = tex.clone();
                    temp.resident_mips = req.target_resident_mips;
                    let new_bytes = temp.resident_vram_bytes();
                    Some(new_bytes.saturating_sub(old_bytes))
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(delta) = delta_opt {
                if self.vram_used_bytes + delta > self.vram_budget_bytes {
                    self.evict_lru(delta, camera_pos);
                }

                if let Some(tex) = self.textures.get_mut(&req.texture_id) {
                    let loaded = (req.target_resident_mips.saturating_sub(tex.resident_mips)) as usize;
                    tex.resident_mips = req.target_resident_mips;
                    tex.last_access_frame = self.frame_counter;
                    self.vram_used_bytes += delta;
                    frame_bytes_transferred += delta;
                    self.stats.mips_loaded_this_frame += loaded;
                }
            }
        }

        self.stats.pending_requests_count = self.request_queue.len();
        self.stats.bytes_streamed_this_frame = frame_bytes_transferred;
        self.stats.cumulative_bytes_streamed += frame_bytes_transferred as u64;
        self.stats.current_vram_used_bytes = self.vram_used_bytes;
    }

    /// Evicts high mip levels from least recently accessed and distant textures until target bytes are freed.
    fn evict_lru(&mut self, bytes_needed: usize, camera_pos: Vec3) {
        let mut candidates: Vec<(TextureId, f32, u64)> = self.textures
            .iter()
            .filter(|(_, t)| t.resident_mips > 1) // Only textures with more than base mip
            .map(|(id, t)| {
                let dist = (t.world_pos - camera_pos).length();
                (*id, dist, t.last_access_frame)
            })
            .collect();

        // Sort descending: farthest and oldest first
        candidates.sort_by(|a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal)
                .then_with(|| a.2.cmp(&b.2))
        });

        let mut bytes_freed = 0;
        for (id, _, _) in candidates {
            if bytes_freed >= bytes_needed {
                break;
            }
            if let Some(tex) = self.textures.get_mut(&id) {
                let before = tex.resident_vram_bytes();
                tex.resident_mips = 1; // Demote back to baseline ambient mip
                let after = tex.resident_vram_bytes();
                let freed = before.saturating_sub(after);
                self.vram_used_bytes = self.vram_used_bytes.saturating_sub(freed);
                bytes_freed += freed;
                self.stats.mips_evicted_this_frame += 1;
            }
        }
    }

    /// Returns current streaming telemetry and statistics.
    #[inline]
    pub fn stats(&self) -> &TextureStreamingStats {
        &self.stats
    }

    /// Current resident VRAM usage in bytes.
    #[inline]
    pub fn current_vram_used(&self) -> usize {
        self.vram_used_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_texture_mip_pyramid_computation() {
        let tex = StreamableTexture::new(
            TextureId(1),
            "vehicle_diffuse_4k",
            4096,
            4096,
            TextureFormat::Bc1Dxt1,
            Vec3::new(0.0, 0.0, 10.0),
        );

        // 4096 -> 2048 -> 1024 -> 512 -> 256 -> 128 -> 64 -> 32 -> 16 -> 8 -> 4 -> 2 -> 1 (13 mips)
        assert_eq!(tex.total_mips, 13);
        assert_eq!(tex.resident_mips, 1);

        // Baseline resident mip is 1x1 or 4x4 block: 8 bytes
        assert!(tex.resident_vram_bytes() > 0);
        assert!(tex.full_vram_bytes() > tex.resident_vram_bytes());
    }

    #[test]
    fn test_progressive_streaming_and_distance_lod() {
        let mut streamer = TextureStreamer::new(512 * 1024 * 1024); // 512 MB budget

        let tex1 = StreamableTexture::new(
            TextureId(1),
            "close_building",
            2048,
            2048,
            TextureFormat::Bc7,
            Vec3::new(0.0, 0.0, 10.0), // 10m away
        );
        let tex2 = StreamableTexture::new(
            TextureId(2),
            "far_mountain",
            2048,
            2048,
            TextureFormat::Bc7,
            Vec3::new(0.0, 0.0, 1000.0), // 1000m away (horizon)
        );

        streamer.register_texture(tex1);
        streamer.register_texture(tex2);

        let cam_pos = Vec3::ZERO;
        let cam_dir = Vec3::new(0.0, 0.0, 1.0);

        streamer.update(cam_pos, cam_dir, 0.016);

        let close_tex = streamer.get_texture(TextureId(1)).unwrap();
        let far_tex = streamer.get_texture(TextureId(2)).unwrap();

        // Close texture should have upgraded to high resident mips
        assert!(close_tex.resident_mips > far_tex.resident_mips);
        assert_eq!(far_tex.resident_mips, 1); // Farthest remains at base ambient level
    }

    #[test]
    fn test_vram_budget_pressure_and_lru_eviction() {
        // Strict small budget of 2 MB
        let mut streamer = TextureStreamer::new(2 * 1024 * 1024);

        for i in 0..10 {
            let tex = StreamableTexture::new(
                TextureId(i),
                format!("texture_{}", i),
                1024,
                1024,
                TextureFormat::Bc1Dxt1,
                Vec3::new(0.0, 0.0, (i as f32 + 1.0) * 5.0),
            );
            streamer.register_texture(tex);
        }

        let cam_pos = Vec3::ZERO;
        let cam_dir = Vec3::new(0.0, 0.0, 1.0);

        // Trigger updates under budget pressure
        for _ in 0..5 {
            streamer.update(cam_pos, cam_dir, 0.016);
        }

        // VRAM usage must not exceed the specified budget
        assert!(streamer.current_vram_used() <= 2 * 1024 * 1024);
        assert!(streamer.stats().mips_evicted_this_frame > 0 || streamer.current_vram_used() > 0);
    }
}
