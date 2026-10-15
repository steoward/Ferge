use ow_math::{Mat4, Quat, Vec3};
use ow_physics::AABB;

/// Camera operation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraMode {
    /// Over-the-shoulder / 3rd person follow for on-foot characters.
    Character,
    /// Trailing behind a vehicle with velocity-dependent lag and FOV stretch.
    Vehicle,
    /// Free inspection orbit.
    FreeOrbit,
}

/// Advanced 3rd-person follow camera with predictive obstacle collision avoidance (RAGE spring-arm).
#[derive(Debug, Clone)]
pub struct OrbitCameraRig {
    pub mode: CameraMode,
    pub target_position: Vec3,
    pub current_position: Vec3,
    pub current_rotation: Quat,
    /// Yaw angle in radians.
    pub yaw: f32,
    /// Pitch angle in radians.
    pub pitch: f32,
    pub min_pitch: f32,
    pub max_pitch: f32,
    /// Desired unobstructed follow distance.
    pub target_distance: f32,
    /// Actual current distance after collision clamping and spring damping.
    pub current_distance: f32,
    pub min_distance: f32,
    pub height_offset: f32,
    pub shoulder_offset: f32,
    /// Damping speed for positional smoothing.
    pub position_damping: f32,
    /// Damping speed for rotational smoothing.
    pub rotation_damping: f32,
    /// Safety sphere radius used when checking obstacle intersections.
    pub collision_radius: f32,
    /// Field of view in degrees.
    pub fov_degrees: f32,
}

impl Default for OrbitCameraRig {
    fn default() -> Self {
        Self::new()
    }
}

impl OrbitCameraRig {
    pub fn new() -> Self {
        Self {
            mode: CameraMode::Character,
            target_position: Vec3::ZERO,
            current_position: Vec3::new(0.0, 2.0, -4.5),
            current_rotation: Quat::IDENTITY,
            yaw: 0.0,
            pitch: 0.2, // Slight downward angle
            min_pitch: -1.2, // ~ -70 degrees
            max_pitch: 1.4,  // ~ +80 degrees
            target_distance: 4.5,
            current_distance: 4.5,
            min_distance: 0.8,
            height_offset: 1.6, // Character head/shoulder level
            shoulder_offset: 0.3,
            position_damping: 12.0,
            rotation_damping: 10.0,
            collision_radius: 0.25,
            fov_degrees: 65.0,
        }
    }

    /// Handles mouse / right stick look input.
    pub fn rotate(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.yaw += delta_yaw;
        self.pitch = (self.pitch + delta_pitch).clamp(self.min_pitch, self.max_pitch);
    }

    /// Computes the look-at pivot point from the target position and offsets.
    pub fn compute_pivot(&self) -> Vec3 {
        let base_pivot = self.target_position + Vec3::new(0.0, self.height_offset, 0.0);
        let right_dir = Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin()).normalize();
        base_pivot + right_dir * self.shoulder_offset
    }

    /// Computes the ideal unobstructed camera position based on yaw, pitch, and target distance.
    pub fn compute_ideal_position(&self, pivot: Vec3, distance: f32) -> Vec3 {
        let cos_p = self.pitch.cos();
        let sin_p = self.pitch.sin();
        let cos_y = self.yaw.cos();
        let sin_y = self.yaw.sin();

        // Direction pointing back toward the camera from the pivot
        let offset_dir = Vec3::new(
            sin_y * cos_p,
            sin_p,
            cos_y * cos_p,
        );

        pivot + offset_dir * distance
    }

    /// Performs raycast obstacle inspection against world bounding boxes to prevent clipping inside walls.
    pub fn resolve_collision_distance(&self, pivot: Vec3, colliders: &[AABB]) -> f32 {
        let ideal_pos = self.compute_ideal_position(pivot, self.target_distance);
        let ray_dir = ideal_pos - pivot;
        let total_dist = ray_dir.length();

        if total_dist < 1e-4 {
            return self.target_distance;
        }

        let dir_norm = ray_dir.normalize();
        let mut nearest_hit = self.target_distance;

        for aabb in colliders {
            let padding = Vec3::new(self.collision_radius, self.collision_radius, self.collision_radius);
            let expanded_min = aabb.min - padding;
            let expanded_max = aabb.max + padding;

            if let Some(t) = intersect_ray_box(pivot, dir_norm, expanded_min, expanded_max) {
                if t >= 0.0 && t < nearest_hit {
                    nearest_hit = t;
                }
            }
        }

        nearest_hit.clamp(self.min_distance, self.target_distance)
    }

    /// Updates camera positioning, damping, and collision resolution each frame.
    pub fn update(
        &mut self,
        dt: f32,
        target_pos: Vec3,
        target_velocity: Vec3,
        world_colliders: &[AABB],
    ) {
        self.target_position = target_pos;

        // In vehicle mode, dynamically adjust follow distance and FOV based on speed
        if self.mode == CameraMode::Vehicle {
            let speed_kmh = target_velocity.length() * 3.6;
            let speed_factor = (speed_kmh / 150.0).clamp(0.0, 1.0);
            self.target_distance = 6.0 + speed_factor * 3.0; // Distance extends at high speeds
            self.fov_degrees = 65.0 + speed_factor * 15.0;  // High-speed FOV rush
        }

        let pivot = self.compute_pivot();

        // Check for physical obstacles between pivot and camera
        let clamped_dist = self.resolve_collision_distance(pivot, world_colliders);

        // Exponential smoothing toward the clamped distance
        let dist_speed = if clamped_dist < self.current_distance {
            // Instant snap forward on collision to avoid popping inside geometry
            25.0
        } else {
            // Smooth release back to full distance
            4.0
        };
        self.current_distance += (clamped_dist - self.current_distance) * (dist_speed * dt).min(1.0);

        let ideal_pos = self.compute_ideal_position(pivot, self.current_distance);

        // Smooth camera translation
        let pos_blend = (self.position_damping * dt).min(1.0);
        self.current_position = self.current_position + (ideal_pos - self.current_position) * pos_blend;

        // Compute rotation matching yaw and pitch
        let target_rot = Quat::from_euler(0.0, self.pitch, self.yaw);
        self.current_rotation = self.current_rotation.slerp(&target_rot, (self.rotation_damping * dt).min(1.0));
    }

    /// Computes the view matrix corresponding to the current camera state.
    pub fn compute_view_matrix(&self) -> Mat4 {
        let pivot = self.compute_pivot();
        Mat4::look_at(self.current_position, pivot, Vec3::UP)
    }
}

/// Helper function implementing ray-AABB slab intersection.
fn intersect_ray_box(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let inv_dir_x = if dir.x.abs() > 1e-6 { 1.0 / dir.x } else { f32::INFINITY };
    let inv_dir_y = if dir.y.abs() > 1e-6 { 1.0 / dir.y } else { f32::INFINITY };
    let inv_dir_z = if dir.z.abs() > 1e-6 { 1.0 / dir.z } else { f32::INFINITY };

    let mut tmin = (min.x - origin.x) * inv_dir_x;
    let mut tmax = (max.x - origin.x) * inv_dir_x;
    if tmin > tmax { std::mem::swap(&mut tmin, &mut tmax); }

    let mut tymin = (min.y - origin.y) * inv_dir_y;
    let mut tymax = (max.y - origin.y) * inv_dir_y;
    if tymin > tymax { std::mem::swap(&mut tymin, &mut tymax); }

    if tmin > tymax || tymin > tmax { return None; }
    if tymin > tmin { tmin = tymin; }
    if tymax < tmax { tmax = tymax; }

    let mut tzmin = (min.z - origin.z) * inv_dir_z;
    let mut tzmax = (max.z - origin.z) * inv_dir_z;
    if tzmin > tzmax { std::mem::swap(&mut tzmin, &mut tzmax); }

    if tmin > tzmax || tzmin > tmax { return None; }
    if tzmin > tmin { tmin = tzmin; }
    if tzmax < tmax { tmax = tzmax; }

    if tmax < 0.0 { return None; }
    Some(if tmin < 0.0 { tmax } else { tmin })
}

