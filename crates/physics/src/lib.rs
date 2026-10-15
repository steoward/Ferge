pub mod body;
pub mod collider;
pub mod world;
pub mod raycast;
pub mod gjk;
pub mod broadphase;

pub use body::{RigidBody, BodyType, BodyShape};
pub use collider::{Collider, ColliderShape, AABB, Sphere, Capsule, Box};
pub use world::{PhysicsWorld, PhysicsConfig, Contact, CollisionEvent};
pub use raycast::{Ray, RaycastHit, RaycastResult};
pub use gjk::{
    collide, gjk, gjk_distance, gjk_intersects, epa_penetration, Contact as NarrowphaseContact,
    SphereSupport, BoxSupport, CapsuleSupport, PointSupport, ConvexHull, SupportGeometry,
    SupportPoint, GjkResult,
};
pub use broadphase::{SpatialHash, Aabb, QueryStats, brute_force_pairs};
