use ow_math::Vec3;
use ow_memory::Pool;
use ow_physics::body::{RigidBody, BodyType};
use ow_physics::collider::{AABB, Sphere, Collider, ColliderShape};
use ow_physics::world::{PhysicsWorld, PhysicsConfig};
use ow_physics::raycast::Ray;
use ow_gameplay::character::CharacterController;
use ow_gameplay::vehicle::VehicleController;
use ow_gameplay::weapon::{Weapon, WeaponType};
use ow_gameplay::ai::{AIController, AIBehavior};
use ow_gameplay::quest::{QuestManager, Quest, Objective, ObjectiveType, QuestState};

#[derive(Debug, PartialEq)]
struct Item {
    value: u32,
}

#[test]
fn pool_insert_get_remove_with_generations() {
    let mut pool: Pool<Item> = Pool::new();
    let a = pool.insert(Item { value: 1 });
    let b = pool.insert(Item { value: 2 });

    assert_eq!(pool.len(), 2);
    assert_eq!(pool.get(a).unwrap().value, 1);
    assert_eq!(pool.get(b).unwrap().value, 2);

    let removed = pool.remove(a).unwrap();
    assert_eq!(removed.value, 1);
    assert_eq!(pool.len(), 1);

    // Stale handle must be rejected after removal
    assert!(pool.get(a).is_none());

    // Recycled slot must not match the stale handle
    let c = pool.insert(Item { value: 3 });
    assert!(pool.get(a).is_none());
    assert_eq!(pool.get(c).unwrap().value, 3);
    assert_eq!(pool.len(), 2);
}

#[test]
fn pool_iter_skips_removed() {
    let mut pool: Pool<Item> = Pool::new();
    let a = pool.insert(Item { value: 1 });
    let _b = pool.insert(Item { value: 2 });
    pool.remove(a);

    let values: Vec<u32> = pool.iter().map(|i| i.value).collect();
    assert_eq!(values, vec![2]);
}

#[test]
fn rigid_body_impulse_respects_inv_mass() {
    let mut body = RigidBody::new(0, BodyType::Dynamic);
    body.set_mass(2.0);
    body.apply_impulse(Vec3::new(4.0, 0.0, 0.0));
    assert!((body.velocity.x - 2.0).abs() < 1e-6);
}

#[test]
fn static_bodies_ignore_forces() {
    let mut body = RigidBody::new(0, BodyType::Static);
    body.apply_force(Vec3::new(100.0, 0.0, 0.0));
    assert_eq!(body.velocity, Vec3::ZERO);
    assert_eq!(body.inv_mass, 0.0);
}

#[test]
fn rigid_body_gravity_integration_moves_down() {
    let mut body = RigidBody::new(0, BodyType::Dynamic);
    body.set_mass(1.0);
    let start = body.position;
    body.integrate(0.5, Vec3::new(0.0, -10.0, 0.0));
    assert!(body.position.y < start.y);
    assert!(body.velocity.y < 0.0);
}

#[test]
fn aabb_intersection_and_containment() {
    let a = AABB::new(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0));
    let b = AABB::new(Vec3::new(1.0, 1.0, 1.0), Vec3::new(3.0, 3.0, 3.0));
    let c = AABB::new(Vec3::new(10.0, 0.0, 0.0), Vec3::new(12.0, 2.0, 2.0));

    assert!(a.intersects(&b));
    assert!(!a.intersects(&c));
    assert!(a.contains_point(Vec3::ONE));
    assert!(!a.contains_point(Vec3::new(5.0, 5.0, 5.0)));
    assert_eq!(a.size(), Vec3::new(2.0, 2.0, 2.0));
}

#[test]
fn sphere_intersection_and_containment() {
    let a = Sphere::new(Vec3::ZERO, 1.0);
    let b = Sphere::new(Vec3::new(1.5, 0.0, 0.0), 1.0);
    let c = Sphere::new(Vec3::new(5.0, 0.0, 0.0), 1.0);

    assert!(a.intersects_sphere(&b));
    assert!(!a.intersects_sphere(&c));
    assert!(a.contains_point(Vec3::new(0.5, 0.0, 0.0)));
    assert!(!a.contains_point(Vec3::new(2.0, 0.0, 0.0)));
}

#[test]
fn physics_world_creates_bodies() {
    let mut world = PhysicsWorld::new(PhysicsConfig::default());
    let a = world.create_body(BodyType::Dynamic);
    let b = world.create_body(BodyType::Static);
    assert_eq!(world.body_count(), 2);

    world.get_body_mut(a).unwrap().set_mass(5.0);
    assert_eq!(world.get_body(a).unwrap().mass, 5.0);
    assert_eq!(world.get_body(b).unwrap().body_type, BodyType::Static);

    world.destroy_body(a);
    assert_eq!(world.body_count(), 1);
    assert!(world.get_body(a).is_none());
}

#[test]
fn ray_normalizes_direction_and_projects_points() {
    let ray = Ray::new(Vec3::ZERO, Vec3::new(0.0, 0.0, 10.0));
    assert!((ray.direction.length() - 1.0).abs() < 1e-6);
    assert_eq!(ray.get_point(5.0), Vec3::new(0.0, 0.0, 5.0));
}

#[test]
fn collider_builder_defaults_and_overrides() {
    let collider = Collider::sphere(2.0)
        .with_offset(Vec3::new(1.0, 0.0, 0.0))
        .with_trigger(true)
        .with_layer(3, 0xF);

    match collider.shape {
        ColliderShape::Sphere(r) => assert_eq!(r, 2.0),
        _ => panic!("expected sphere"),
    }
    assert_eq!(collider.offset, Vec3::new(1.0, 0.0, 0.0));
    assert!(collider.is_trigger);
    assert_eq!(collider.layer, 3);
    assert_eq!(collider.mask, 0xF);
}

#[test]
fn character_moves_and_reports_state() {
    let mut player = CharacterController::new(1, "Hero");
    let forward = Vec3::new(0.0, 0.0, 1.0);
    for _ in 0..10 {
        player.update(1.0 / 60.0, forward, false);
    }

    assert!(player.position.0.z > 0.0, "player should move forward");
    assert!(player.speed > 0.0);
    assert!(
        matches!(
            player.state,
            ow_gameplay::character::CharacterState::Walking
                | ow_gameplay::character::CharacterState::Running
        ),
        "expected a moving state, got {:?}",
        player.state
    );
}

#[test]
fn character_jump_impulse_leaves_ground() {
    let mut player = CharacterController::new(1, "Jumper");
    player.update(0.1, Vec3::ZERO, true);
    assert!(player.velocity.0.y > 0.0);
    assert!(!player.is_grounded);
}

#[test]
fn character_dies_at_zero_health() {
    let mut player = CharacterController::new(1, "Doomed");
    player.take_damage(100.0);
    assert!(!player.is_alive());
    assert!(!player.can_move);
    assert!(matches!(
        player.state,
        ow_gameplay::character::CharacterState::Dead
    ));
}

#[test]
fn vehicle_accelerates_and_steers() {
    let mut car = VehicleController::new(7);
    car.start_engine();
    car.accelerate(1.0);

    for _ in 0..120 {
        car.update(1.0 / 60.0);
    }
    assert!(car.speed > 0.0, "car should gain speed");
    assert!(car.get_speed_kmh() > 0.0);
}

#[test]
fn vehicle_brakes_to_stop() {
    let mut car = VehicleController::new(8);
    car.start_engine();
    car.accelerate(1.0);
    for _ in 0..60 {
        car.update(1.0 / 60.0);
    }
    let rolling_speed = car.speed;
    assert!(rolling_speed > 0.0);

    car.accelerate(0.0);
    car.brake(1.0);
    for _ in 0..120 {
        car.update(1.0 / 60.0);
    }
    assert!(car.speed < rolling_speed);
}

#[test]
fn weapon_fires_and_depletes_magazine() {
    let mut weapon = Weapon::new("Pistol", WeaponType::Pistol);
    weapon.magazine_size = 3;
    weapon.current_ammo = 3;

    assert!(weapon.can_fire());
    for _ in 0..3 {
        assert!(weapon.fire());
    }
    assert!(!weapon.can_fire());
    assert_eq!(weapon.current_ammo, 0);
    assert!(!weapon.fire());
}

#[test]
fn weapon_reload_restores_from_reserve() {
    let mut weapon = Weapon::new("SMG", WeaponType::SMG);
    weapon.magazine_size = 10;
    weapon.current_ammo = 2;
    weapon.reserve_ammo = 30;
    weapon.reload_time = 1.0;

    assert!(weapon.reload());
    for _ in 0..70 {
        weapon.update(1.0 / 60.0);
    }
    assert_eq!(weapon.current_ammo, 10);
    assert_eq!(weapon.reserve_ammo, 22);
    assert!(weapon.can_fire());
}

#[test]
fn ai_aggressive_chases_then_attacks() {
    let mut ai = AIController::new(20);
    ai.behavior = AIBehavior::Aggressive;

    // Player far away: AI must chase
    ai.update(1.0 / 60.0, Vec3::new(0.0, 0.0, 10.0));
    assert!(matches!(ai.state, ow_gameplay::ai::AIState::Chase));
    assert!(ai.is_alerted);

    let start = ai.position.0;
    ai.update(0.5, Vec3::new(0.0, 0.0, 10.0));
    assert_ne!(ai.position.0, start, "AI should move while chasing");

    // Player in attack range: AI must switch to Attack
    ai.update(1.0 / 60.0, ai.position.0);
    assert!(matches!(ai.state, ow_gameplay::ai::AIState::Attack));
}

#[test]
fn ai_coward_flees_when_threat_approaches() {
    let mut ai = AIController::new(22);
    ai.behavior = AIBehavior::Coward;
    ai.update(1.0 / 60.0, Vec3::new(0.0, 0.0, 5.0));
    assert!(matches!(ai.state, ow_gameplay::ai::AIState::Flee));
    assert!(ai.is_alerted);
}

#[test]
fn ai_passive_stays_idle_without_patrol() {
    let mut ai = AIController::new(23);
    ai.behavior = AIBehavior::Passive;
    ai.update(1.0 / 60.0, Vec3::ZERO);
    assert!(matches!(ai.state, ow_gameplay::ai::AIState::Idle));
    assert!(!ai.is_alerted);
}

#[test]
fn ai_dead_state_when_health_zero() {
    let mut ai = AIController::new(21);
    ai.health.take_damage(100.0);
    ai.update(1.0 / 60.0, Vec3::ZERO);
    assert!(matches!(ai.state, ow_gameplay::ai::AIState::Dead));
}

#[test]
fn quest_completes_when_all_objectives_done() {
    let mut manager = QuestManager::new();
    let mut quest = Quest::new(1, "Tutorial", "Learn the ropes");
    quest.objectives.push(Objective::new(
        1,
        ObjectiveType::Reach,
        "Reach the marker",
        1,
    ));
    manager.add_quest(quest);

    assert!(manager.start_quest(1));
    assert_eq!(manager.active_count(), 1);

    if let Some(q) = manager.get_quest_mut(1) {
        q.objectives[0].update_progress(1);
        assert!(q.objectives[0].is_completed);
    }

    manager.update(1.0 / 60.0);
    assert_eq!(manager.completed_count(), 1);
    assert_eq!(manager.active_count(), 0);
    assert_eq!(
        manager.get_quest(1).unwrap().state,
        QuestState::Completed
    );
}

#[test]
fn quest_fails_on_timeout() {
    let mut manager = QuestManager::new();
    let mut quest = Quest::new(2, "Timed", "Too late");
    quest.objectives.push(Objective::new(
        1,
        ObjectiveType::Collect,
        "Collect items",
        5,
    ));
    quest.time_limit = Some(1.0);
    manager.add_quest(quest);
    manager.start_quest(2);

    manager.update(2.0);
    assert_eq!(manager.get_quest(2).unwrap().state, QuestState::Failed);
}
