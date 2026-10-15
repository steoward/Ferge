use ow_math::Vec3;
use ow_physics::AABB;
use ow_world::camera::OrbitCameraRig;
use ow_world::environment::{EnvironmentSystem, TimeOfDay, WeatherPreset};
use ow_world::population::{
    PopulationConfig, PopulationManager, RoadNetwork,
};
use ow_world::streaming::{
    ChunkLod, SectorId, StreamingBudget, StreamingConfig, StreamingManager, WorldChunk,
};
use ow_gameplay::handling::{DrivetrainType, FourWheelVehicle, HandlingConfig};
use ow_gameplay::script::{ScriptContext, ScriptManager, ScriptYield};

// ---------------------------------------------------------------------------
// 1. World Streaming Subsystem Tests
// ---------------------------------------------------------------------------

#[test]
fn sector_id_conversions_and_bounds() {
    let sector_size = 200.0;
    let pos = Vec3::new(450.0, 0.0, -150.0);
    let id = SectorId::from_world_pos(pos, sector_size);

    assert_eq!(id.x, 2);
    assert_eq!(id.z, -1);

    let center = id.to_world_center(sector_size);
    assert_eq!(center.x, 500.0);
    assert_eq!(center.z, -100.0);

    let bounds = id.to_bounds(sector_size, -50.0, 150.0);
    assert_eq!(bounds.min, Vec3::new(400.0, -50.0, -200.0));
    assert_eq!(bounds.max, Vec3::new(600.0, 150.0, 0.0));
}

#[test]
fn streaming_manager_loads_near_chunks_and_predicts_velocity() {
    let mut config = StreamingConfig::default();
    config.sector_size = 100.0;
    config.high_lod_radius = 120.0;
    config.medium_lod_radius = 250.0;
    config.max_streaming_radius = 600.0;
    config.velocity_anticipation_seconds = 2.0;

    let budget = StreamingBudget {
        max_memory_bytes: 100 * 1024 * 1024,
        current_memory_bytes: 0,
        max_loading_per_frame: 10,
    };

    let mut manager = StreamingManager::new(config, budget);

    // Register a 5x5 grid of chunks around origin
    for x in -2..=2 {
        for z in -2..=2 {
            let id = SectorId::new(x, z);
            let chunk = WorldChunk::new(id, 100.0, 0.0, 50.0, 1024 * 1024); // 1 MB each
            manager.register_chunk(chunk);
        }
    }

    assert_eq!(manager.total_chunks(), 25);
    assert_eq!(manager.active_chunk_count(), 0);

    // Player standing still at origin
    manager.update(Vec3::new(0.0, 0.0, 0.0), Vec3::ZERO, 1);
    assert!(manager.active_chunk_count() > 0, "Near chunks should load");

    // Center chunk should be High LOD
    let center_chunk = manager.get_chunk(SectorId::new(0, 0)).unwrap();
    assert!(center_chunk.is_resident());
    assert_eq!(center_chunk.current_lod, ChunkLod::High);

    // Player driving fast forward (+Z at 50 m/s = 180 km/h)
    // Predictive focus point should lead by 50 * 2.0 = 100m along +Z
    let forward_velocity = Vec3::new(0.0, 0.0, 50.0);
    let focus = manager.predict_focus_point(Vec3::ZERO, forward_velocity);
    assert_eq!(focus.z, 100.0);

    manager.update(Vec3::ZERO, forward_velocity, 2);
    let ahead_chunk = manager.get_chunk(SectorId::new(0, 1)).unwrap();
    assert!(ahead_chunk.is_resident(), "Ahead chunk must be loaded due to velocity anticipation");
}

#[test]
fn streaming_budget_eviction_under_pressure() {
    let config = StreamingConfig {
        sector_size: 50.0,
        high_lod_radius: 60.0,
        medium_lod_radius: 120.0,
        low_lod_radius: 200.0,
        max_streaming_radius: 300.0,
        velocity_anticipation_seconds: 0.0,
    };

    let budget = StreamingBudget {
        max_memory_bytes: 3 * 1024 * 1024, // Only 3 MB allowed in budget
        current_memory_bytes: 0,
        max_loading_per_frame: 10,
    };

    let mut manager = StreamingManager::new(config, budget);

    // Register 10 chunks, 1 MB each
    for x in 0..10 {
        let id = SectorId::new(x, 0);
        let chunk = WorldChunk::new(id, 50.0, 0.0, 10.0, 1024 * 1024);
        manager.register_chunk(chunk);
    }

    manager.update(Vec3::new(25.0, 0.0, 25.0), Vec3::ZERO, 1);

    // Memory usage should not exceed the 3 MB quota
    assert!(
        manager.budget.current_memory_bytes <= manager.budget.max_memory_bytes,
        "Current memory {} exceeded budget {}",
        manager.budget.current_memory_bytes,
        manager.budget.max_memory_bytes
    );
}

// ---------------------------------------------------------------------------
// 2. Atmosphere & Environment Subsystem Tests
// ---------------------------------------------------------------------------

#[test]
fn time_of_day_cycle_and_celestial_trajectory() {
    let mut time = TimeOfDay::new(12.0, 0.0, 0.0); // Noon
    assert!(time.is_daytime());
    assert!(!time.is_nighttime());

    let (sun_noon, moon_noon) = time.compute_celestial_vectors();
    assert!(sun_noon.y > 0.8, "Sun should be high overhead at noon");
    assert!(moon_noon.y < -0.8, "Moon should be below horizon at noon");

    // Advance 12 hours to midnight
    time.hours = 0.0;
    assert!(!time.is_daytime());
    assert!(time.is_nighttime());

    let (sun_midnight, _moon_midnight) = time.compute_celestial_vectors();
    assert!(sun_midnight.y < -0.8, "Sun should be below horizon at midnight");
    assert!(moon_noon.dot(&sun_noon) < -0.99, "Moon and sun must oppose each other");
}

#[test]
fn dynamic_weather_transitions_and_wetness() {
    let mut env = EnvironmentSystem::new();
    env.time.hours = 12.0;
    env.set_weather(WeatherPreset::Rain, 1.0); // 1 second transition

    // Simulate 2 seconds of heavy rain
    for _ in 0..120 {
        env.update(1.0 / 60.0);
    }

    let atmo = env.evaluate_atmosphere();
    assert_eq!(env.current_weather, WeatherPreset::Rain);
    assert!(atmo.rain_intensity > 0.5, "Rain intensity should be high");
    assert!(atmo.surface_wetness > 0.05, "Puddles should accumulate wetness");

    // Switch to ExtraSunny to dry off
    env.set_weather(WeatherPreset::ExtraSunny, 0.5);
    for _ in 0..180 {
        env.update(1.0 / 60.0);
    }

    let dried_atmo = env.evaluate_atmosphere();
    assert!(dried_atmo.surface_wetness < atmo.surface_wetness, "Sun must evaporate wetness");
    assert!(dried_atmo.sun_intensity > atmo.sun_intensity, "Sunny sky has higher intensity");
}

// ---------------------------------------------------------------------------
// 3. Orbit Camera Rig & Obstacle Avoidance Tests
// ---------------------------------------------------------------------------

#[test]
fn camera_unobstructed_positioning_and_view_matrix() {
    let mut cam = OrbitCameraRig::new();
    cam.target_distance = 5.0;
    cam.yaw = 0.0;
    cam.pitch = 0.0;

    let target = Vec3::new(10.0, 0.0, 10.0);
    cam.update(0.1, target, Vec3::ZERO, &[]);

    let view = cam.compute_view_matrix();
    // Non-zero determinant / valid transformation matrix
    assert!(view.m[0] != 0.0 || view.m[5] != 0.0);
}

#[test]
fn camera_collision_clamping_prevents_wall_clipping() {
    let mut cam = OrbitCameraRig::new();
    cam.target_distance = 6.0;
    cam.yaw = 0.0;
    cam.pitch = 0.0;
    cam.shoulder_offset = 0.0;

    let target = Vec3::ZERO;
    // Place a solid wall AABB right behind the player at Z = 2.5 (camera wants to be at Z = 6.0)
    let wall = AABB::new(
        Vec3::new(-5.0, -1.0, 2.0),
        Vec3::new(5.0, 4.0, 3.0),
    );

    let colliders = vec![wall];

    // Update with wall present
    for _ in 0..30 {
        cam.update(1.0 / 60.0, target, Vec3::ZERO, &colliders);
    }

    // Camera distance should be clamped in front of the wall (well under 6.0)
    assert!(
        cam.current_distance < 3.0,
        "Camera must clamp forward before wall, got dist {}",
        cam.current_distance
    );
    assert!(
        cam.current_distance >= cam.min_distance,
        "Camera must respect min distance"
    );
}

// ---------------------------------------------------------------------------
// 4. Ambient Population & Traffic Subsystem Tests
// ---------------------------------------------------------------------------

#[test]
fn road_network_traffic_and_ambient_spawning() {
    let mut network = RoadNetwork::new();
    // Main avenue from Z=0 to Z=500
    network.add_node(1, Vec3::new(0.0, 0.0, 0.0), false);
    network.add_node(2, Vec3::new(0.0, 0.0, 500.0), false);
    network.add_lane(100, 1, 2, 60.0); // 60 km/h

    let config = PopulationConfig {
        spawn_inner_radius: 30.0,
        spawn_outer_radius: 80.0,
        despawn_radius: 120.0,
        max_vehicles: 5,
        max_pedestrians: 5,
    };

    let mut pop_mgr = PopulationManager::new(config, network);

    // Player moving forward along avenue
    let player_pos = Vec3::new(0.0, 0.0, 50.0);
    let player_fwd = Vec3::new(0.0, 0.0, 1.0);

    // Trigger update to allow spawn timer to fire
    for _ in 0..70 {
        pop_mgr.update(1.0 / 60.0, player_pos, player_fwd);
    }

    assert!(pop_mgr.vehicle_count() > 0, "Ambient vehicles should spawn on lane");
    assert!(pop_mgr.pedestrian_count() > 0, "Ambient pedestrians should spawn");

    // Teleport player far away to test despawning
    let distant_pos = Vec3::new(2000.0, 0.0, 2000.0);
    pop_mgr.update(1.0 / 60.0, distant_pos, player_fwd);

    assert_eq!(pop_mgr.vehicle_count(), 0, "Vehicles should despawn outside radius");
    assert_eq!(pop_mgr.pedestrian_count(), 0, "Pedestrians should despawn outside radius");
}

// ---------------------------------------------------------------------------
// 5. 4-Wheel Vehicle Physics & Handling Tests
// ---------------------------------------------------------------------------

#[test]
fn four_wheel_vehicle_suspension_and_acceleration() {
    let handling = HandlingConfig {
        mass: 1400.0,
        drivetrain: DrivetrainType::RearWheelDrive,
        drive_force: 6000.0,
        suspension_stiffness: 40000.0,
        suspension_comp_damping: 3500.0,
        ..Default::default()
    };

    let mut car = FourWheelVehicle::new(handling);
    car.position = Vec3::new(0.0, 1.0, 0.0); // Spawn slightly elevated

    // Step physics onto ground plane at Y = 0.0
    for _ in 0..60 {
        car.update(1.0 / 60.0, 0.0);
    }

    // Suspension should support the vehicle above the floor
    assert!(car.position.y > 0.25, "Vehicle must stay above floor on springs");
    assert!(car.wheels[0].is_grounded, "Front-left wheel should be grounded");
    assert!(car.wheels[2].is_grounded, "Rear-left wheel should be grounded");

    // Apply throttle
    car.set_inputs(1.0, 0.0, false, 0.0);
    for _ in 0..180 {
        car.update(1.0 / 60.0, 0.0);
    }

    assert!(car.get_speed_kmh() > 20.0, "Car should accelerate forward");
    assert!(car.engine_rpm > 1200.0, "Engine RPM must rise under load");
}

#[test]
fn four_wheel_vehicle_handbrake_triggers_drift_slip() {
    let mut car = FourWheelVehicle::new(HandlingConfig::default());
    car.position = Vec3::new(0.0, 0.4, 0.0);

    // Accelerate to speed
    car.set_inputs(1.0, 0.0, false, 0.0);
    for _ in 0..100 {
        car.update(1.0 / 60.0, 0.0);
    }

    let pre_drift_speed = car.get_speed_kmh();
    assert!(pre_drift_speed > 15.0);

    // Pull handbrake and turn wheel hard
    car.set_inputs(0.0, 0.0, true, 1.0);
    for _ in 0..20 {
        car.update(1.0 / 60.0, 0.0);
    }

    // Rear wheels should exhibit high skid / drift intensity
    assert!(
        car.wheels[2].skid_intensity > 0.8,
        "Rear left wheel should break traction on handbrake"
    );
    assert!(
        car.wheels[3].skid_intensity > 0.8,
        "Rear right wheel should break traction on handbrake"
    );
}

// ---------------------------------------------------------------------------
// 6. Cooperative Script Fiber Scheduler Tests
// ---------------------------------------------------------------------------

#[test]
fn script_fiber_wait_frames_and_shared_globals() {
    let mut script_mgr = ScriptManager::new();

    let mut state = 0;
    script_mgr.spawn_fiber("MissionStep", move |ctx: &mut ScriptContext| {
        match state {
            0 => {
                ctx.set_global("MISSION_STAGE", 1);
                state = 1;
                ScriptYield::WaitFrames(3)
            }
            1 => {
                ctx.set_global("MISSION_STAGE", 2);
                state = 2;
                ScriptYield::WaitFrames(2)
            }
            _ => {
                ctx.set_global("MISSION_STAGE", 3);
                ScriptYield::Done
            }
        }
    });

    assert_eq!(script_mgr.active_fiber_count(), 1);

    // Frame 1
    script_mgr.update(1.0 / 60.0);
    assert_eq!(script_mgr.context.get_global("MISSION_STAGE"), 1);

    // Frame 2 & 3 (waiting)
    script_mgr.update(1.0 / 60.0);
    assert_eq!(script_mgr.context.get_global("MISSION_STAGE"), 1);
    script_mgr.update(1.0 / 60.0);
    assert_eq!(script_mgr.context.get_global("MISSION_STAGE"), 1);

    // Frame 4 (resumes -> stage 2)
    script_mgr.update(1.0 / 60.0);
    assert_eq!(script_mgr.context.get_global("MISSION_STAGE"), 2);
}

#[test]
fn script_fiber_wait_until_condition() {
    let mut script_mgr = ScriptManager::new();

    script_mgr.context.set_global("HOSTAGES_SAVED", 0);

    let mut completed = false;
    script_mgr.spawn_fiber("HostageRescue", move |ctx: &mut ScriptContext| {
        if !completed {
            completed = true;
            ScriptYield::WaitUntil(Box::new(|ctx| ctx.get_global("HOSTAGES_SAVED") >= 3))
        } else {
            ctx.set_global("MISSION_WON", 1);
            ScriptYield::Done
        }
    });

    // While hostages < 3, script stays waiting
    script_mgr.update(0.1);
    assert_eq!(script_mgr.context.get_global("MISSION_WON"), 0);

    // Set hostages to 3
    script_mgr.context.set_global("HOSTAGES_SAVED", 3);
    script_mgr.update(0.1);

    assert_eq!(script_mgr.context.get_global("MISSION_WON"), 1);
    assert_eq!(script_mgr.active_fiber_count(), 0, "Finished fiber is cleaned up");
}
