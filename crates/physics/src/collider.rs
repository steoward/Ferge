use ow_math::{Vec3, Quat};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColliderShape {
    Sphere(f32),
    Box(Vec3),
    Capsule(f32, f32),
    Plane(Vec3, f32),
}

#[derive(Debug, Clone)]
pub struct Collider {
    pub shape: ColliderShape,
    pub offset: Vec3,
    pub rotation: Quat,
    pub is_trigger: bool,
    pub layer: u32,
    pub mask: u32,
}

impl Collider {
    pub fn new(shape: ColliderShape) -> Self {
        Self {
            shape,
            offset: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            is_trigger: false,
            layer: 0,
            mask: u32::MAX,
        }
    }

    pub fn sphere(radius: f32) -> Self {
        Self::new(ColliderShape::Sphere(radius))
    }

    pub fn collider_box(half_extents: Vec3) -> Self {
        Self::new(ColliderShape::Box(half_extents))
    }

    pub fn capsule(radius: f32, half_height: f32) -> Self {
        Self::new(ColliderShape::Capsule(radius, half_height))
    }

    pub fn plane(normal: Vec3, distance: f32) -> Self {
        Self::new(ColliderShape::Plane(normal, distance))
    }

    pub fn with_offset(mut self, offset: Vec3) -> Self {
        self.offset = offset;
        self
    }

    pub fn with_rotation(mut self, rotation: Quat) -> Self {
        self.rotation = rotation;
        self
    }

    pub fn with_trigger(mut self, is_trigger: bool) -> Self {
        self.is_trigger = is_trigger;
        self
    }

    pub fn with_layer(mut self, layer: u32, mask: u32) -> Self {
        self.layer = layer;
        self.mask = mask;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AABB {
    pub min: Vec3,
    pub max: Vec3,
}

impl AABB {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn from_center_size(center: Vec3, size: Vec3) -> Self {
        let half = size * 0.5;
        Self {
            min: center - half,
            max: center + half,
        }
    }

    pub fn contains_point(&self, point: Vec3) -> bool {
        point.x >= self.min.x && point.x <= self.max.x &&
        point.y >= self.min.y && point.y <= self.max.y &&
        point.z >= self.min.z && point.z <= self.max.z
    }

    pub fn intersects(&self, other: &Self) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x &&
        self.min.y <= other.max.y && self.max.y >= other.min.y &&
        self.min.z <= other.max.z && self.max.z >= other.min.z
    }

    pub fn expand(&mut self, point: Vec3) {
        self.min.x = self.min.x.min(point.x);
        self.min.y = self.min.y.min(point.y);
        self.min.z = self.min.z.min(point.z);
        self.max.x = self.max.x.max(point.x);
        self.max.y = self.max.y.max(point.y);
        self.max.z = self.max.z.max(point.z);
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sphere {
    pub center: Vec3,
    pub radius: f32,
}

impl Sphere {
    pub fn new(center: Vec3, radius: f32) -> Self {
        Self { center, radius }
    }

    pub fn contains_point(&self, point: Vec3) -> bool {
        (point - self.center).length() <= self.radius
    }

    pub fn intersects_sphere(&self, other: &Self) -> bool {
        (self.center - other.center).length() <= self.radius + other.radius
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Capsule {
    pub point_a: Vec3,
    pub point_b: Vec3,
    pub radius: f32,
}

impl Capsule {
    pub fn new(point_a: Vec3, point_b: Vec3, radius: f32) -> Self {
        Self { point_a, point_b, radius }
    }

    pub fn closest_point_on_segment(&self, point: Vec3) -> Vec3 {
        let ab = self.point_b - self.point_a;
        let t = ((point - self.point_a).dot(&ab) / ab.dot(&ab)).clamp(0.0, 1.0);
        self.point_a + ab * t
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Box {
    pub center: Vec3,
    pub half_extents: Vec3,
}

impl Box {
    pub fn new(center: Vec3, half_extents: Vec3) -> Self {
        Self { center, half_extents }
    }

    pub fn contains_point(&self, point: Vec3) -> bool {
        let local = point - self.center;
        local.x.abs() <= self.half_extents.x &&
        local.y.abs() <= self.half_extents.y &&
        local.z.abs() <= self.half_extents.z
    }
}
