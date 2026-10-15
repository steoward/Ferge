use ow_math::{Vec3, Quat, Transform};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position(pub Vec3);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rotation(pub Quat);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scale(pub Vec3);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Velocity(pub Vec3);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformComponent(pub Transform);

#[derive(Debug, Clone, PartialEq)]
pub struct Name(pub String);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    pub fn new(max: f32) -> Self {
        Self { current: max, max }
    }

    pub fn is_alive(&self) -> bool {
        self.current > 0.0
    }

    pub fn take_damage(&mut self, damage: f32) {
        self.current = (self.current - damage).max(0.0);
    }

    pub fn heal(&mut self, amount: f32) {
        self.current = (self.current + amount).min(self.max);
    }

    pub fn ratio(&self) -> f32 {
        if self.max > 0.0 {
            self.current / self.max
        } else {
            0.0
        }
    }
}
