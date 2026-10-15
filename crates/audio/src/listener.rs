use ow_math::Vec3;

#[derive(Debug, Clone)]
pub struct AudioListener {
    pub position: Vec3,
    pub forward: Vec3,
    pub up: Vec3,
    pub velocity: Vec3,
    pub volume: f32,
}

impl AudioListener {
    pub fn new() -> Self {
        Self {
            position: Vec3::ZERO,
            forward: Vec3::FORWARD,
            up: Vec3::UP,
            velocity: Vec3::ZERO,
            volume: 1.0,
        }
    }

    pub fn with_position(mut self, position: Vec3) -> Self {
        self.position = position;
        self
    }

    pub fn with_orientation(mut self, forward: Vec3, up: Vec3) -> Self {
        self.forward = forward.normalize();
        self.up = up.normalize();
        self
    }

    pub fn with_velocity(mut self, velocity: Vec3) -> Self {
        self.velocity = velocity;
        self
    }

    pub fn with_volume(mut self, volume: f32) -> Self {
        self.volume = volume.clamp(0.0, 1.0);
        self
    }

    pub fn get_right(&self) -> Vec3 {
        self.forward.cross(self.up).normalize()
    }
}

impl Default for AudioListener {
    fn default() -> Self {
        Self::new()
    }
}
