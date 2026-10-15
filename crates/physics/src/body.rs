use ow_math::{Vec3, Quat};
use crate::collider::ColliderShape;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BodyType {
    Static,
    Kinematic,
    Dynamic,
}

#[derive(Debug, Clone)]
pub struct RigidBody {
    pub id: u32,
    pub body_type: BodyType,
    pub position: Vec3,
    pub rotation: Quat,
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
    pub mass: f32,
    pub inv_mass: f32,
    pub restitution: f32,
    pub friction: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub gravity_scale: f32,
    pub is_sleeping: bool,
    pub sleep_threshold: f32,
    pub collider_shape: Option<ColliderShape>,
    pub user_data: u64,
}

impl RigidBody {
    pub fn new(id: u32, body_type: BodyType) -> Self {
        let mass = if body_type == BodyType::Dynamic { 1.0 } else { 0.0 };
        Self {
            id,
            body_type,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            mass,
            inv_mass: if mass > 0.0 { 1.0 / mass } else { 0.0 },
            restitution: 0.0,
            friction: 0.5,
            linear_damping: 0.01,
            angular_damping: 0.01,
            gravity_scale: 1.0,
            is_sleeping: false,
            sleep_threshold: 0.1,
            collider_shape: None,
            user_data: 0,
        }
    }

    pub fn set_mass(&mut self, mass: f32) {
        self.mass = mass.max(0.0);
        self.inv_mass = if self.mass > 0.0 { 1.0 / self.mass } else { 0.0 };
    }

    pub fn apply_force(&mut self, force: Vec3) {
        if self.body_type != BodyType::Dynamic || self.is_sleeping {
            return;
        }
        self.velocity = self.velocity + force * self.inv_mass;
    }

    pub fn apply_impulse(&mut self, impulse: Vec3) {
        if self.body_type != BodyType::Dynamic || self.is_sleeping {
            return;
        }
        self.velocity = self.velocity + impulse * self.inv_mass;
    }

    pub fn apply_torque(&mut self, torque: Vec3) {
        if self.body_type != BodyType::Dynamic || self.is_sleeping {
            return;
        }
        self.angular_velocity = self.angular_velocity + torque * self.inv_mass;
    }

    pub fn integrate(&mut self, dt: f32, gravity: Vec3) {
        if self.body_type != BodyType::Dynamic || self.is_sleeping {
            return;
        }

        let gravity_force = gravity * self.gravity_scale;
        self.velocity = self.velocity + gravity_force * dt;
        self.velocity = self.velocity * (1.0 - self.linear_damping);
        self.angular_velocity = self.angular_velocity * (1.0 - self.angular_damping);

        self.position = self.position + self.velocity * dt;

        if self.angular_velocity.length() > 0.0 {
            let angle = self.angular_velocity.length() * dt;
            let axis = self.angular_velocity.normalize();
            let delta_rotation = Quat::from_axis_angle(axis, angle);
            self.rotation = (delta_rotation * self.rotation).normalize();
        }

        if self.velocity.length() < self.sleep_threshold {
            self.is_sleeping = true;
        }
    }

    pub fn wake_up(&mut self) {
        self.is_sleeping = false;
    }

    pub fn get_aabb(&self) -> Option<(Vec3, Vec3)> {
        self.collider_shape.as_ref().map(|shape| {
            match shape {
                ColliderShape::Sphere(radius) => (
                    self.position - Vec3::new(*radius, *radius, *radius),
                    self.position + Vec3::new(*radius, *radius, *radius),
                ),
                ColliderShape::Box(half_extents) => (
                    self.position - *half_extents,
                    self.position + *half_extents,
                ),
                ColliderShape::Capsule(radius, half_height) => {
                    let r = *radius;
                    let h = *half_height + r;
                    (
                        self.position - Vec3::new(r, h, r),
                        self.position + Vec3::new(r, h, r),
                    )
                }
                ColliderShape::Plane(normal, distance) => {
                    let n = normal.normalize();
                    let extent = Vec3::new(1000.0, 1.0, 1000.0);
                    let center = n * *distance;
                    let point_a = center - extent;
                    let point_b = center + extent;
                    (
                        Vec3::new(point_a.x.min(point_b.x), point_a.y.min(point_b.y), point_a.z.min(point_b.z)),
                        Vec3::new(point_a.x.max(point_b.x), point_a.y.max(point_b.y), point_a.z.max(point_b.z)),
                    )
                }
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyShape {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}
