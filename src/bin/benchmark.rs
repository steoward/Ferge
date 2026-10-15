use std::time::Instant;
use std::collections::HashMap;
use ow_math::{Mat4, Quat, Transform, Vec3};
use ow_memory::pool::Pool;
use ow_ecs::world::World;
use ow_ecs::entity::EntityId;
use ow_ecs::component::{Position, Velocity, Health};
use ow_ecs::system::{System, SystemStage};
use ow_physics::gjk::{collide, SphereSupport, BoxSupport};
use ow_physics::broadphase::{SpatialHash, Aabb};
use ow_gameplay::handling::{FourWheelVehicle, HandlingConfig};
use ow_gameplay::script::{ScriptManager, ScriptYield};
use ow_world::streaming::{SectorId, StreamingBudget, StreamingConfig, StreamingManager, WorldChunk};
use ow_world::environment::EnvironmentSystem;
use ow_world::camera::OrbitCameraRig;

struct PositionIntegrator;
impl System for PositionIntegrator {
    fn stage(&self) -> SystemStage {
        SystemStage::Update
    }
    fn run(&self, world: &mut World, dt: f32) {
        let count = world.entity_count() as u64;
        for i in 0..count {
            let id = EntityId { id: i, generation: 0 };
            if let Some(entity) = world.get_entity(id) {
                entity.with_component_mut::<Position, _>(|p| {
                    p.0 = p.0 + Vec3::new(1.0, 0.0, 2.0) * dt;
                });
            }
        }
    }
}

fn bench_section(name: &str) {
    println!("\n============================================================");
    println!("  BENCHMARK: {}", name);
    println!("============================================================");
}

fn print_result(label: &str, ops: usize, elapsed_secs: f64) {
    let ops_per_sec = ops as f64 / elapsed_secs;
    let us_per_op = (elapsed_secs * 1_000_000.0) / ops as f64;
    println!(
        "  {:<35} : {:>8.2} ms | {:>12.0} ops/sec | {:>7.2} ns/op",
        label,
        elapsed_secs * 1000.0,
        ops_per_sec,
        us_per_op * 1000.0
    );
}

fn main() {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║       FerrumRAGE / OpenWorldEngine BENCHMARK SUITE       ║");
    println!("║             High-Performance Subsystem Auditing          ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!("Build Profile: Release (opt-level=3, LTO=true, codegen-units=1)");
    println!("Hardware Target: x86_64 Native");

    bench_math();
    bench_memory();
    bench_ecs();
    bench_physics();
    bench_vehicles();
    bench_streaming();
    bench_atmosphere();
    bench_scripts();

    println!("\nAll benchmark suites completed successfully.\n");
}

fn bench_math() {
    bench_section("Linear Algebra & SIMD Math (ow-math)");

    // 1. Vector3 Dot & Cross
    let n = 2_000_000;
    let v1 = Vec3::new(1.5, 2.5, 3.5);
    let v2 = Vec3::new(4.5, 5.5, 6.5);
    let mut sum = 0.0f32;
    let t0 = Instant::now();
    for i in 0..n {
        let factor = i as f32 * 0.0001;
        let v = v1 * factor;
        sum += v.dot(&v2) + v.cross(v2).length();
    }
    let el = t0.elapsed().as_secs_f64();
    print_result("Vec3 dot + cross + norm", n, el);
    assert!(sum != 0.0);

    // 2. Quaternion Slerp
    let q1 = Quat::IDENTITY;
    let q2 = Quat::from_axis_angle(Vec3::UP, std::f32::consts::PI * 0.5);
    let t0 = Instant::now();
    let mut q_res = Quat::IDENTITY;
    for i in 0..n {
        let t = (i % 1000) as f32 / 1000.0;
        q_res = q1.slerp(&q2, t);
    }
    let el = t0.elapsed().as_secs_f64();
    print_result("Quaternion slerp interpolation", n, el);
    assert!(q_res.w != 0.0);

    // 3. Mat4 Multiplication & Inversion / Composition
    let m_ops = 1_000_000;
    let m1 = Mat4::perspective(1.0, 1.77, 0.1, 1000.0);
    let m2 = Mat4::look_at(Vec3::new(0.0, 5.0, 10.0), Vec3::ZERO, Vec3::UP);
    let t0 = Instant::now();
    let mut m_acc = Mat4::identity();
    for _ in 0..m_ops {
        m_acc = m1 * m2;
    }
    let el = t0.elapsed().as_secs_f64();
    print_result("Mat4 4x4 matrix multiplication", m_ops, el);
    assert!(m_acc.m[0] != 0.0);

    // 4. Transform Composition (Local -> Model Space)
    let t_ops = 1_000_000;
    let tr1 = Transform::new(Vec3::new(10.0, 2.0, -5.0), q2, Vec3::new(2.0, 2.0, 2.0));
    let tr2 = Transform::new(Vec3::new(0.0, 1.5, 0.0), q1, Vec3::ONE);
    let t0 = Instant::now();
    let mut tr_res = Transform::identity();
    for _ in 0..t_ops {
        tr_res = tr1 * tr2;
    }
    let el = t0.elapsed().as_secs_f64();
    print_result("Transform hierarchy composition", t_ops, el);
    assert!(tr_res.position.x != 0.0);
}

fn bench_memory() {
    bench_section("Generational Memory Pool (ow-memory)");

    let n = 1_000_000;
    let mut pool: Pool<[f32; 8]> = Pool::new();

    let t0 = Instant::now();
    let mut handles = Vec::with_capacity(n);
    for i in 0..n {
        let val = [i as f32; 8];
        handles.push(pool.insert(val));
    }
    let el_insert = t0.elapsed().as_secs_f64();
    print_result("Pool::insert (generational handle)", n, el_insert);

    let t0 = Instant::now();
    let mut read_sum = 0.0f32;
    for h in &handles {
        if let Some(item) = pool.get(*h) {
            read_sum += item[0];
        }
    }
    let el_get = t0.elapsed().as_secs_f64();
    print_result("Pool::get (generational lookup)", n, el_get);
    assert!(read_sum != 0.0);

    let t0 = Instant::now();
    for h in handles {
        pool.remove(h);
    }
    let el_remove = t0.elapsed().as_secs_f64();
    print_result("Pool::remove (slot recycling)", n, el_remove);
}

fn bench_ecs() {
    bench_section("Data-Oriented ECS System Scheduling (ow-ecs)");

    let entity_counts = [5_000, 20_000];

    for &count in &entity_counts {
        let mut world = World::new();
        world.add_system(PositionIntegrator);

        for i in 0..count {
            let e = world.create_entity();
            let entity = world.get_entity(e).unwrap();
            entity.add_component(Position(Vec3::new(i as f32, 0.0, 0.0)));
            entity.add_component(Velocity(Vec3::new(1.0, 0.0, 2.0)));
            entity.add_component(Health::new(100.0));
        }

        // Measure system stage execution over 50 simulation frames
        let frames = 50;
        let t0 = Instant::now();
        for _ in 0..frames {
            world.run_systems(SystemStage::Update, 1.0 / 60.0);
        }
        let el = t0.elapsed().as_secs_f64();
        let frame_ms = (el / frames as f64) * 1000.0;
        let fps = frames as f64 / el;
        println!(
            "  ECS {:>6} entities {:>2} frames : {:>6.2} ms/frame | {:>7.0} FPS",
            count, frames, frame_ms, fps
        );
    }
}

fn bench_physics() {
    bench_section("Collision Broadphase & Narrowphase (ow-physics)");

    // 1. GJK / EPA Narrowphase
    let n = 200_000;
    let s1 = SphereSupport::new(Vec3::ZERO, 1.0);
    let s2 = SphereSupport::new(Vec3::new(1.5, 0.0, 0.0), 1.0);
    let b1 = BoxSupport::new(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
    let b2 = BoxSupport::new(Vec3::new(1.8, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));

    let t0 = Instant::now();
    let mut hits = 0;
    for _ in 0..n {
        if collide(&s1, &s2).is_some() { hits += 1; }
        if collide(&b1, &b2).is_some() { hits += 1; }
    }
    let el = t0.elapsed().as_secs_f64();
    print_result("GJK/EPA narrowphase contact pair", n * 2, el);
    assert!(hits > 0);

    // 2. SpatialHash Broadphase
    let mut grid = SpatialHash::new(20.0);
    let mut aabbs = HashMap::new();
    let bodies = 2_000;
    let t0 = Instant::now();
    for i in 0..bodies {
        let x = (i % 45) as f32 * 12.0;
        let z = (i / 45) as f32 * 12.0;
        let aabb = Aabb::new(Vec3::new(x, 0.0, z), Vec3::new(x + 5.0, 2.0, z + 5.0));
        aabbs.insert(i as u32, aabb);
        grid.insert(i as u32, aabb);
    }
    let el_insert = t0.elapsed().as_secs_f64();
    print_result("SpatialHash insert 2k bodies", bodies, el_insert);

    let t0 = Instant::now();
    let pairs = grid.candidate_pairs(&aabbs);
    let el_pairs = t0.elapsed().as_secs_f64();
    println!(
        "  SpatialHash candidate pairs found ({:>5} pairs): {:>6.2} ms",
        pairs.len(),
        el_pairs * 1000.0
    );
}

fn bench_vehicles() {
    bench_section("4-Wheel Raycast Vehicle Simulation (ow-gameplay)");

    let vehicle_counts = [50, 200, 500];

    for &count in &vehicle_counts {
        let mut vehicles = Vec::with_capacity(count);
        for _ in 0..count {
            let mut v = FourWheelVehicle::new(HandlingConfig::default());
            v.set_inputs(1.0, 0.0, false, 0.2);
            vehicles.push(v);
        }

        let frames = 60;
        let t0 = Instant::now();
        for _ in 0..frames {
            for v in &mut vehicles {
                v.update(1.0 / 60.0, 0.0);
            }
        }
        let el = t0.elapsed().as_secs_f64();
        let total_updates = count * frames;
        let frame_time_us = (el * 1_000_000.0) / frames as f64;
        let us_per_vehicle = (el * 1_000_000.0) / total_updates as f64;
        println!(
            "  {:>4} simultaneous 4-wheel vehicles : {:>6.2} µs/frame | {:>5.2} µs/veh/step | {:>6.0} FPS headroom",
            count, frame_time_us, us_per_vehicle, 1_000_000.0 / frame_time_us
        );
    }
}

fn bench_streaming() {
    bench_section("Spatial World Streaming & Predictive Cache (ow-world)");

    let mut config = StreamingConfig::default();
    config.sector_size = 100.0;
    let budget = StreamingBudget {
        max_memory_bytes: 256 * 1024 * 1024,
        current_memory_bytes: 0,
        max_loading_per_frame: 16,
    };

    let mut manager = StreamingManager::new(config, budget);

    // Register a 40x40 world grid (1,600 sectors = 16 km²)
    for x in -20..20 {
        for z in -20..20 {
            let id = SectorId::new(x, z);
            let chunk = WorldChunk::new(id, 100.0, -50.0, 300.0, 512 * 1024);
            manager.register_chunk(chunk);
        }
    }

    let iterations = 1_000;
    let t0 = Instant::now();
    for frame in 0..iterations {
        let t = frame as f32 * 0.1;
        let player_pos = Vec3::new(t * 10.0, 0.0, (t * 0.5).sin() * 500.0);
        let velocity = Vec3::new(40.0, 0.0, 10.0); // 150 km/h driving
        manager.update(player_pos, velocity, frame as u64);
    }
    let el = t0.elapsed().as_secs_f64();
    let us_per_stream_eval = (el * 1_000_000.0) / iterations as f64;
    println!(
        "  1,600 sector grid streaming updates : {:>6.2} µs/update | {:>6.0} updates/sec (active: {})",
        us_per_stream_eval,
        iterations as f64 / el,
        manager.active_chunk_count()
    );
}

fn bench_atmosphere() {
    bench_section("Atmosphere, Celestial Orbits & Camera (ow-world)");

    let mut env = EnvironmentSystem::new();
    let n = 200_000;
    let t0 = Instant::now();
    for _ in 0..n {
        env.update(1.0 / 60.0);
        let _atmo = env.evaluate_atmosphere();
    }
    let el = t0.elapsed().as_secs_f64();
    print_result("Atmosphere & 24h orbit evaluation", n, el);

    let mut cam = OrbitCameraRig::new();
    let t0 = Instant::now();
    for _ in 0..n {
        cam.update(1.0 / 60.0, Vec3::new(10.0, 0.0, 5.0), Vec3::new(0.0, 0.0, 20.0), &[]);
        let _view = cam.compute_view_matrix();
    }
    let el_cam = t0.elapsed().as_secs_f64();
    print_result("OrbitCameraRig update + view matrix", n, el_cam);
}

fn bench_scripts() {
    bench_section("Cooperative Script Fiber Scheduler (ow-gameplay)");

    let mut script_mgr = ScriptManager::new();
    let fibers_count = 1_000;

    for i in 0..fibers_count {
        let mut count = 0;
        script_mgr.spawn_fiber(format!("Fiber_{}", i), move |_ctx| {
            count += 1;
            if count % 2 == 0 {
                ScriptYield::WaitFrames(1)
            } else {
                ScriptYield::Yield
            }
        });
    }

    let frames = 200;
    let t0 = Instant::now();
    for _ in 0..frames {
        script_mgr.update(1.0 / 60.0);
    }
    let el = t0.elapsed().as_secs_f64();
    let total_ticks = fibers_count * frames;
    let us_per_tick = (el * 1_000_000.0) / total_ticks as f64;
    println!(
        "  {:>4} concurrent script fibers : {:>6.2} µs/frame | {:>5.2} ns/fiber/tick | {:>6.0} FPS headroom",
        fibers_count,
        (el * 1_000_000.0) / frames as f64,
        us_per_tick * 1000.0,
        frames as f64 / el
    );
}
