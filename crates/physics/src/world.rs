use std::collections::HashMap;
use ow_math::Vec3;
use crate::body::{RigidBody, BodyType};

#[derive(Debug, Clone)]
pub struct PhysicsConfig {
    pub gravity: Vec3,
    pub time_step: f32,
    pub velocity_iterations: usize,
    pub position_iterations: usize,
    pub max_bodies: usize,
    pub broadphase_grid_size: f32,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            gravity: Vec3::new(0.0, -9.81, 0.0),
            time_step: 1.0 / 60.0,
            velocity_iterations: 8,
            position_iterations: 3,
            max_bodies: 10000,
            broadphase_grid_size: 100.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Contact {
    pub body_a: u32,
    pub body_b: u32,
    pub point: Vec3,
    pub normal: Vec3,
    pub penetration: f32,
    pub restitution: f32,
    pub friction: f32,
}

#[derive(Debug, Clone)]
pub struct CollisionEvent {
    pub body_a: u32,
    pub body_b: u32,
    pub contact: Contact,
    pub is_trigger: bool,
}

pub type CollisionCallback = Box<dyn Fn(&CollisionEvent) + Send + Sync>;

pub struct PhysicsWorld {
    config: PhysicsConfig,
    bodies: HashMap<u32, RigidBody>,
    next_body_id: u32,
    contacts: Vec<Contact>,
    collision_events: Vec<CollisionEvent>,
    collision_callbacks: Vec<CollisionCallback>,
}

impl PhysicsWorld {
    pub fn new(config: PhysicsConfig) -> Self {
        Self {
            config,
            bodies: HashMap::new(),
            next_body_id: 0,
            contacts: Vec::new(),
            collision_events: Vec::new(),
            collision_callbacks: Vec::new(),
        }
    }

    pub fn create_body(&mut self, body_type: BodyType) -> u32 {
        let id = self.next_body_id;
        self.next_body_id += 1;
        let body = RigidBody::new(id, body_type);
        self.bodies.insert(id, body);
        id
    }

    pub fn destroy_body(&mut self, id: u32) {
        self.bodies.remove(&id);
    }

    pub fn get_body(&self, id: u32) -> Option<&RigidBody> {
        self.bodies.get(&id)
    }

    pub fn get_body_mut(&mut self, id: u32) -> Option<&mut RigidBody> {
        self.bodies.get_mut(&id)
    }

    pub fn add_collision_callback<F>(&mut self, callback: F)
    where
        F: Fn(&CollisionEvent) + Send + Sync + 'static,
    {
        self.collision_callbacks.push(Box::new(callback));
    }

    pub fn step(&mut self, dt: f32) {
        let sub_steps = (dt / self.config.time_step).ceil() as usize;
        let sub_dt = dt / sub_steps as f32;

        for _ in 0..sub_steps {
            self.integrate(sub_dt);
            self.broad_phase();
            self.narrow_phase();
            self.resolve_contacts();
        }

        for event in &self.collision_events {
            for callback in &self.collision_callbacks {
                callback(event);
            }
        }

        self.contacts.clear();
        self.collision_events.clear();
    }

    fn integrate(&mut self, dt: f32) {
        for body in self.bodies.values_mut() {
            body.integrate(dt, self.config.gravity);
        }
    }

    fn broad_phase(&mut self) {
        // Simple grid-based broadphase
        // In a production engine, use a spatial hash or BVH
    }

    fn narrow_phase(&mut self) {
        let body_ids: Vec<u32> = self.bodies.keys().copied().collect();

        for i in 0..body_ids.len() {
            for j in (i + 1)..body_ids.len() {
                let id_a = body_ids[i];
                let id_b = body_ids[j];

                if let (Some(body_a), Some(body_b)) = (self.bodies.get(&id_a), self.bodies.get(&id_b)) {
                    if body_a.body_type == BodyType::Static && body_b.body_type == BodyType::Static {
                        continue;
                    }

                    if let Some(contact) = self.check_collision(body_a, body_b) {
                        self.contacts.push(contact.clone());
                        self.collision_events.push(CollisionEvent {
                            body_a: id_a,
                            body_b: id_b,
                            contact,
                            is_trigger: false,
                        });
                    }
                }
            }
        }
    }

    fn check_collision(&self, _body_a: &RigidBody, _body_b: &RigidBody) -> Option<Contact> {
        // Simplified collision detection
        // In a production engine, use GJK/EPA for convex shapes
        None
    }

    fn resolve_contacts(&mut self) {
        for contact in &self.contacts {
            let normal = contact.normal;

            let (body_a, body_b) = (contact.body_a, contact.body_b);
            let (a_vel, b_vel) = (
                self.bodies.get(&body_a).map(|b| b.velocity),
                self.bodies.get(&body_b).map(|b| b.velocity),
            );

            let (impulse, correction) = match (a_vel, b_vel) {
                (Some(a), Some(b)) => {
                    let vel_along_normal = (a - b).dot(&normal);
                    if vel_along_normal > 0.0 {
                        (None, Vec3::ZERO)
                    } else {
                        let j = -(1.0 + contact.restitution) * vel_along_normal;
                        let correction =
                            normal * (contact.penetration - 0.01).max(0.0) * 0.2;
                        (Some(normal * j), correction)
                    }
                }
                _ => (None, Vec3::ZERO),
            };

            if let Some(body_a) = self.bodies.get_mut(&body_a) {
                if let Some(impulse) = impulse {
                    body_a.apply_impulse(impulse);
                }
                body_a.position = body_a.position + correction;
            }

            if let Some(body_b) = self.bodies.get_mut(&body_b) {
                if let Some(impulse) = impulse {
                    body_b.apply_impulse(-impulse);
                }
                body_b.position = body_b.position - correction;
            }
        }
    }

    pub fn get_contacts(&self) -> &[Contact] {
        &self.contacts
    }

    pub fn get_collision_events(&self) -> &[CollisionEvent] {
        &self.collision_events
    }

    pub fn body_count(&self) -> usize {
        self.bodies.len()
    }
}
