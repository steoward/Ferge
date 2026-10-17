use std::path::Path;
use ow_core::engine::{Engine, EngineConfig};
use ow_math::{Mat4, Quat, Transform, Vec3, Vec4};
use ow_ecs::component::Position;
use ow_gameplay::character::CharacterController;
use ow_gameplay::handling::{DrivetrainType, FourWheelVehicle, HandlingConfig};
use ow_gameplay::weapon::{Weapon, WeaponType};
use ow_gameplay::ai::{AIController, AIBehavior};
use ow_gameplay::quest::{QuestManager, Quest, Objective, ObjectiveType};
use ow_gameplay::script::{ScriptContext, ScriptManager, ScriptYield};
use ow_physics::AABB;
use ow_world::camera::OrbitCameraRig;
use ow_world::environment::{EnvironmentSystem, WeatherPreset};
use ow_world::population::{PopulationConfig, PopulationManager, RoadNetwork};
use ow_world::streaming::{
    SectorId, StreamingBudget, StreamingConfig, StreamingManager, WorldChunk,
};
use ow_asset::rsc7::{create_synthetic_rsc7_drawable, Rsc7Drawable};
use ow_render::mesh::{Mesh, Vertex};
use ow_render::renderer::{Renderer, RenderConfig as OwRenderConfig};
use ow_render::camera::Camera;
use ow_render::light::{Light, LightType};
use ow_render::rasterizer::{SoftwareRasterPass, ShadingModel};
use ow_render::pass::RenderPass;

fn main() {
    println!("============================================================");
    println!("                  Ferge (Fe Rust Game Engine)               ");
    println!("============================================================");
    println!("Architecture: Composable 13-Crate Data-Oriented Engine");
    println!("Features: Streaming, 4-Wheel Raycast Physics, Atmosphere,");
    println!("          Orbit Camera, Ambient Crowd/Traffic, Script Fibers");
    println!();

    let config = EngineConfig {
        window_width: 1920,
        window_height: 1080,
        window_title: "Ferge Engine (Fe Rust Game Engine - Clean-Room Open-World Engine)".to_string(),
        vsync: true,
        target_fps: 60,
        enable_physics: true,
        enable_audio: true,
        enable_networking: false,
        log_level: ow_core::engine::LogLevel::Info,
    };

    let engine = Engine::new(config);

    println!("Engine initialized successfully!");
    println!("Window: {}x{}", engine.get_config().window_width, engine.get_config().window_height);
    println!("Target FPS: {}", engine.get_config().target_fps);
    println!();

    let scene = engine.get_scene();
    {
        let scene = scene.write();
        let cam_id = scene.create_node("MainCamera");
        if let Some(mut node) = scene.get_node_mut(cam_id) {
            node.transform = Transform::new(
                Vec3::new(0.0, 5.0, 10.0),
                Quat::from_euler(0.0, 0.0, 0.0),
                Vec3::ONE,
            );
        }
        let player_id = scene.create_node("Player");
        if let Some(mut node) = scene.get_node_mut(player_id) {
            node.transform = Transform::new(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE);
        }
    }

    // Run deep subsystem verification
    verify_gameplay_systems();
    verify_open_world_systems();
    demonstrate_3d_pipeline();

    println!("Starting engine loop (10 frames full simulation)...");
    for _ in 0..10 {
        engine.update(1.0 / 60.0);
    }
    println!("Frames simulated: {}", engine.get_frame_count());
    println!("FPS: {:.1}", engine.get_fps());
    println!();

    println!("Ferge (Fe Rust Game Engine) execution completed successfully.");
}

fn verify_gameplay_systems() {
    println!("--- Core Gameplay Systems ---");

    let mut player = CharacterController::new(1, "Player");
    println!(
        "Character: {} hp={}/{} state={:?}",
        player.name.0, player.health.current, player.health.max, player.state
    );
    player.update(1.0 / 60.0, Vec3::new(0.0, 0.0, 1.0), false);
    player.take_damage(30.0);
    player.heal(10.0);
    println!(
        "  after move+damage: hp={} state={:?} pos={:?}",
        player.health.current, player.state, player.position.0
    );

    let mut rifle = Weapon::new("Carbine Rifle", WeaponType::Rifle);
    for _ in 0..5 {
        rifle.fire();
    }
    rifle.update(1.0 / 60.0);
    println!(
        "Weapon: {} ammo={} reserve={} state={:?}",
        rifle.name, rifle.current_ammo, rifle.reserve_ammo, rifle.state
    );
    rifle.reload();
    for _ in 0..200 {
        rifle.update(1.0 / 60.0);
    }
    println!(
        "  after reload: ammo={} reserve={}",
        rifle.current_ammo, rifle.reserve_ammo
    );

    let mut npc = AIController::new(3);
    npc.behavior = AIBehavior::Aggressive;
    npc.position = Position(Vec3::new(0.0, 0.0, 5.0));
    npc.update(1.0 / 60.0, Vec3::ZERO);
    println!("AI: state={:?} alerted={}", npc.state, npc.is_alerted);

    let mut quests = QuestManager::new();
    let mut quest = Quest::new(1, "Grand Heist", "Secure the convoy");
    quest.objectives.push(Objective::new(1, ObjectiveType::Reach, "Reach safehouse", 1));
    quests.add_quest(quest);
    quests.start_quest(1);
    if let Some(q) = quests.get_quest_mut(1) {
        q.objectives[0].update_progress(1);
    }
    quests.update(1.0 / 60.0);
    println!("Quests: active={} completed={}", quests.active_count(), quests.completed_count());
    println!();
}

fn verify_open_world_systems() {
    println!("--- Open-World RAGE-Equivalent Subsystems ---");

    // 1. World Streaming with Predictive Velocity Loading
    let stream_cfg = StreamingConfig::default();
    let stream_budget = StreamingBudget::default();
    let mut streaming = StreamingManager::new(stream_cfg, stream_budget);

    for x in -5..=5 {
        for z in -5..=5 {
            let id = SectorId::new(x, z);
            let chunk = WorldChunk::new(id, 200.0, -100.0, 400.0, 2 * 1024 * 1024);
            streaming.register_chunk(chunk);
        }
    }
    let player_car_velocity = Vec3::new(0.0, 0.0, 45.0); // ~162 km/h
    streaming.update(Vec3::ZERO, player_car_velocity, 1);
    println!(
        "World Streaming: registered={} active_sectors={} memory_used={:.2} MB",
        streaming.total_chunks(),
        streaming.active_chunk_count(),
        streaming.budget.current_memory_bytes as f32 / (1024.0 * 1024.0)
    );

    // 2. 4-Wheel Raycast Vehicle Physics
    let handling = HandlingConfig {
        mass: 1450.0,
        drivetrain: DrivetrainType::AllWheelDrive,
        drive_force: 5800.0,
        ..Default::default()
    };
    let mut vehicle = FourWheelVehicle::new(handling);
    vehicle.set_inputs(1.0, 0.0, false, 0.1);
    for _ in 0..120 {
        vehicle.update(1.0 / 60.0, 0.0);
    }
    println!(
        "4-Wheel Vehicle: speed={:.1} km/h gear={} rpm={:.0} FL_susp={:.2}m skid={:.2}",
        vehicle.get_speed_kmh(),
        vehicle.current_gear,
        vehicle.engine_rpm,
        vehicle.wheels[0].current_suspension_length,
        vehicle.wheels[2].skid_intensity
    );

    // 3. Orbit Camera with Obstacle Collision Clamping
    let mut camera_rig = OrbitCameraRig::new();
    let obstacle_wall = AABB::new(Vec3::new(-5.0, 0.0, 2.0), Vec3::new(5.0, 5.0, 3.0));
    camera_rig.update(0.1, Vec3::ZERO, player_car_velocity, &[obstacle_wall]);
    println!(
        "Orbit Camera: pos=({:.1}, {:.1}, {:.1}) clamped_dist={:.2}m fov={:.1}°",
        camera_rig.current_position.x,
        camera_rig.current_position.y,
        camera_rig.current_position.z,
        camera_rig.current_distance,
        camera_rig.fov_degrees
    );

    // 4. Atmosphere & Dynamic Weather System
    let mut env = EnvironmentSystem::new();
    env.time.hours = 18.5; // Sunset
    env.set_weather(WeatherPreset::Rain, 2.0);
    for _ in 0..120 {
        env.update(1.0 / 60.0);
    }
    let atmo = env.evaluate_atmosphere();
    println!(
        "Atmosphere: time={:02.0}:{:02.0} weather={:?} sun_elev={:.2} rain={:.1} wetness={:.2}",
        env.time.hours,
        env.time.minutes,
        env.current_weather,
        atmo.sun_direction.y,
        atmo.rain_intensity,
        atmo.surface_wetness
    );

    // 5. Ambient Population & Traffic
    let mut road_net = RoadNetwork::new();
    road_net.add_node(1, Vec3::new(0.0, 0.0, 0.0), false);
    road_net.add_node(2, Vec3::new(0.0, 0.0, 600.0), false);
    road_net.add_lane(10, 1, 2, 70.0);
    let mut pop_mgr = PopulationManager::new(PopulationConfig::default(), road_net);
    for _ in 0..70 {
        pop_mgr.update(1.0 / 60.0, Vec3::new(0.0, 0.0, 100.0), Vec3::FORWARD);
    }
    println!(
        "Population: ambient_vehicles={} ambient_peds={}",
        pop_mgr.vehicle_count(),
        pop_mgr.pedestrian_count()
    );

    // 6. Script Fiber Cooperative Runner
    let mut scripts = ScriptManager::new();
    scripts.spawn_fiber("ConvoyAmbushMission", |ctx: &mut ScriptContext| {
        ctx.set_global("MISSION_STEP", 1);
        ctx.emit_event("SPAWN_GETAWAY_CAR");
        ScriptYield::WaitSeconds(5.0)
    });
    scripts.update(0.1);
    println!(
        "Script VM: active_fibers={} mission_step={} last_event={:?}",
        scripts.active_fiber_count(),
        scripts.context.get_global("MISSION_STEP"),
        scripts.context.triggered_events.last()
    );
    println!();
}

fn demonstrate_3d_pipeline() {
    println!("--- 3D RSC7 Model Decoding & Software Rasterizer Pipeline ---");

    // 1. Synthesize and decode a GTA 5 3D Drawable (.ydr)
    let car_bytes = create_synthetic_rsc7_drawable("adder_chassis.ydr", "car");
    let drawable = Rsc7Drawable::from_bytes(&car_bytes).expect("Failed to parse RSC7 car drawable");

    println!(
        "RSC7 Drawable Loaded: {} | AABB min=({:.1}, {:.1}, {:.1}) max=({:.1}, {:.1}, {:.1})",
        drawable.name,
        drawable.bounding_box_min.x, drawable.bounding_box_min.y, drawable.bounding_box_min.z,
        drawable.bounding_box_max.x, drawable.bounding_box_max.y, drawable.bounding_box_max.z
    );

    // 2. Convert to Ferge Render Mesh
    let mut all_vertices = Vec::new();
    let mut all_indices = Vec::new();
    if let Some(lod) = &drawable.lod_high {
        for geom in &lod.submeshes {
            let base_vertex = all_vertices.len() as u32;
            for v in geom.to_vertex_buffer() {
                all_vertices.push(
                    Vertex::new(v.position)
                        .with_normal(v.normal)
                        .with_uv(v.uv0)
                        .with_color(v.color)
                );
            }
            for idx in geom.to_indices() {
                all_indices.push(base_vertex + idx);
            }
        }
    }
    let mesh = Mesh::new(&drawable.name)
        .with_vertices(all_vertices)
        .with_indices(all_indices);
    println!(
        "  Mesh Transformed: {} vertices | {} triangles in index buffer",
        mesh.vertex_count(),
        mesh.triangle_count()
    );

    // 3. Setup Renderer and Camera
    let width = 640;
    let height = 480;
    let config = OwRenderConfig {
        width,
        height,
        clear_color: Vec4::new(0.08, 0.12, 0.18, 1.0),
        ..Default::default()
    };
    let renderer = Renderer::new(config);

    let mut camera = Camera::new_perspective(
        Vec3::new(0.0, 2.5, 6.0),
        60.0f32.to_radians(),
        0.1,
        100.0,
    );
    camera.view_matrix = Mat4::look_at(Vec3::new(0.0, 2.5, 6.0), Vec3::new(0.0, 0.5, 0.0), Vec3::UP);
    camera.projection_matrix = Mat4::perspective(60.0f32.to_radians(), width as f32 / height as f32, 0.1, 100.0);
    camera.view_projection_matrix = camera.projection_matrix * camera.view_matrix;
    renderer.add_camera(camera);

    let sun_light = Light::new("Sun", LightType::Directional)
        .with_direction(Vec3::new(0.6, -1.0, 0.4))
        .with_color(Vec3::new(1.0, 0.95, 0.85));
    renderer.add_light(sun_light);

    let _mesh_id = renderer.register_mesh(mesh);

    // 4. Execute SoftwareRasterPass and render frame
    let pass = SoftwareRasterPass::new(width, height);
    pass.rasterizer().write().shading_model = ShadingModel::Phong;
    pass.execute(&renderer);

    let stats = pass.rasterizer().read().stats();
    println!(
        "  Software Rasterizer: rendered {} triangles | processed {} fragments | passed Z-test {}",
        stats.triangles_rendered, stats.fragments_processed, stats.fragments_passed
    );

    // 5. Export rendered frame to BMP
    let _ = std::fs::create_dir_all("docs");
    let output_bmp = Path::new("docs/render_frame.bmp");
    if let Ok(()) = pass.export_frame_bmp(output_bmp) {
        println!("  Rendered Frame Exported: {} (640x480 24-bit BMP)", output_bmp.display());
    }
    println!();
}
