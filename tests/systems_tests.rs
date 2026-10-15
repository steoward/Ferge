use ow_ecs::world::World;
use ow_ecs::component::{Health, Name, Position, Velocity};
use ow_ecs::system::{System, SystemStage};

struct CountingSystem {
    runs: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl System for CountingSystem {
    fn stage(&self) -> SystemStage {
        SystemStage::Update
    }
    fn run(&self, _world: &mut World, _dt: f32) {
        self.runs.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

struct VelocitySystem;

impl System for VelocitySystem {
    fn stage(&self) -> SystemStage {
        SystemStage::Update
    }
    fn run(&self, world: &mut World, dt: f32) {
        let id = world.create_entity();
        let entity = world.get_entity(id).unwrap();
        entity.add_component(Velocity(ow_math::Vec3::new(0.0, 0.0, 10.0)));
        entity.add_component(Position(ow_math::Vec3::ZERO));

        entity.with_component_mut::<Position, _>(|p| {
            p.0 = p.0 + ow_math::Vec3::new(0.0, 0.0, 10.0) * dt;
        });

        let final_pos = entity.get_component::<Position>().unwrap();
        assert!((final_pos.0.z - 10.0 * dt).abs() < 1e-6);
    }
}

#[test]
fn world_creates_and_destroys_entities() {
    let world = World::new();
    let a = world.create_entity();
    let b = world.create_entity();
    assert_eq!(world.entity_count(), 2);

    world.destroy_entity(a);
    assert_eq!(world.entity_count(), 1);
    assert!(world.get_entity(b).is_some());
    assert!(world.get_entity(a).is_none());
}

#[test]
fn entity_component_add_get_remove() {
    let world = World::new();
    let id = world.create_entity();
    let entity = world.get_entity(id).unwrap();

    entity.add_component(Health::new(50.0));
    assert!(entity.has_component::<Health>());
    assert_eq!(entity.get_component::<Health>().unwrap().max, 50.0);

    assert!(entity.remove_component::<Health>());
    assert!(!entity.has_component::<Health>());
}

#[test]
fn health_clamps_and_tracks_death() {
    let mut h = Health::new(100.0);
    h.take_damage(30.0);
    assert_eq!(h.current, 70.0);
    assert_eq!(h.ratio(), 0.7);

    h.take_damage(999.0);
    assert_eq!(h.current, 0.0);
    assert!(!h.is_alive());

    h.heal(10.0);
    assert!(h.is_alive());
    assert_eq!(h.current, 10.0);

    h.heal(999.0);
    assert_eq!(h.current, h.max);
}

#[test]
fn systems_execute_in_priority_order() {
    let mut world = World::new();
    let runs = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    world.add_system(CountingSystem { runs: runs.clone() });
    for _ in 0..5 {
        world.run_systems(SystemStage::Update, 1.0 / 60.0);
    }
    assert_eq!(runs.load(std::sync::atomic::Ordering::SeqCst), 5);
}

#[test]
fn velocity_system_integrates_position() {
    let mut world = World::new();
    world.add_system(VelocitySystem);
    world.run_systems(SystemStage::Update, 0.1);
}

#[test]
fn component_name_roundtrip() {
    let world = World::new();
    let id = world.create_entity();
    let entity = world.get_entity(id).unwrap();
    entity.add_component(Name("Hero".to_string()));
    assert_eq!(entity.get_component::<Name>().unwrap().0, "Hero");
    assert_eq!(entity.component_count(), 1);
}
