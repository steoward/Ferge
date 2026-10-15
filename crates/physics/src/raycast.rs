use ow_math::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
    pub max_distance: f32,
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
            max_distance: f32::INFINITY,
        }
    }

    pub fn with_max_distance(mut self, max_distance: f32) -> Self {
        self.max_distance = max_distance;
        self
    }

    pub fn get_point(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RaycastHit {
    pub point: Vec3,
    pub normal: Vec3,
    pub distance: f32,
    pub body_id: u32,
    pub triangle_index: u32,
}

impl RaycastHit {
    pub fn new(point: Vec3, normal: Vec3, distance: f32, body_id: u32) -> Self {
        Self {
            point,
            normal,
            distance,
            body_id,
            triangle_index: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RaycastResult {
    pub hits: Vec<RaycastHit>,
}

impl RaycastResult {
    pub fn new() -> Self {
        Self { hits: Vec::new() }
    }

    pub fn add_hit(&mut self, hit: RaycastHit) {
        self.hits.push(hit);
    }

    pub fn is_empty(&self) -> bool {
        self.hits.is_empty()
    }

    pub fn len(&self) -> usize {
        self.hits.len()
    }

    pub fn closest(&self) -> Option<&RaycastHit> {
        self.hits.iter().min_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap())
    }

    pub fn sort_by_distance(&mut self) {
        self.hits.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap());
    }
}
