use std::time::Instant;
use std::collections::HashMap;
use std::path::Path;
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
use ow_asset::rpf::{RpfArchive, create_synthetic_rpf};
use ow_asset::rsc7::{create_synthetic_rsc7_drawable, Rsc7Drawable};
use ow_render::mesh::Vertex;
use ow_render::rasterizer::{FrameBuffer, Rasterizer, ShadingModel};
use ow_render::texture_streamer::{TextureStreamer, StreamableTexture, TextureId, TextureFormat};

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
    println!("║               Ferge Engine BENCHMARK SUITE               ║");
    println!("║          High-Performance Subsystem Auditing             ║");
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
    bench_texture_streaming();
    bench_rsc7();
    bench_rasterizer();

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

    let config = StreamingConfig {
        sector_size: 100.0,
        ..Default::default()
    };
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

fn bench_texture_streaming() {
    bench_section("RPFv7 Archive IO & Progressive Texture Streaming (ow-asset / ow-render)");

    // 1. Local GTA 5 Archive Direct IO Benchmarking
    let gta5_rpf = Path::new(r"D:\Games\Grand Theft Auto V\mods\x64e.rpf");
    if gta5_rpf.exists() {
        println!("  Target GTA 5 Archive: D:\\Games\\Grand Theft Auto V\\mods\\x64e.rpf");
        let t0 = Instant::now();
        let mut archive = RpfArchive::open(gta5_rpf).expect("Failed to open GTA 5 archive");
        let open_time = t0.elapsed().as_secs_f64();
        println!(
            "  RPF Table of Contents parsed : {:>8.2} ms | {} files | {} folders | Encryption: {:?}",
            open_time * 1000.0,
            archive.total_files(),
            archive.total_directories(),
            archive.encryption()
        );

        // Benchmark raw streaming bandwidth reading entries from the local GTA 5 archive
        let mut total_bytes = 0usize;
        let t0 = Instant::now();
        let entries_to_test = ["levels/gta5/icons.rpf", "levels/gta5/ped_props.rpf", "levels/gta5/minimap.rpf", "levels/gta5/paths.rpf"];
        for path in &entries_to_test {
            if let Ok(bytes) = archive.read_raw_by_path(path) {
                total_bytes += bytes.len();
            }
        }
        let el = t0.elapsed().as_secs_f64();
        let mb = total_bytes as f64 / (1024.0 * 1024.0);
        let mb_per_sec = mb / el;
        println!(
            "  GTA 5 Local Archive Stream   : {:>8.2} ms | {:>8.2} MB read | {:>10.1} MB/s throughput",
            el * 1000.0,
            mb,
            mb_per_sec
        );
    } else {
        println!("  GTA 5 not detected at target path; running synthetic RPF7 container benchmark.");
        let raw_chunk = vec![0xABu8; 1024 * 1024]; // 1 MB chunk
        let synth_bytes = create_synthetic_rpf(&[
            ("diffuse_tex_01.dds", &raw_chunk, false),
            ("diffuse_tex_02.dds", &raw_chunk, false),
            ("diffuse_tex_03.dds", &raw_chunk, false),
            ("diffuse_tex_04.dds", &raw_chunk, false),
        ]);
        let mut archive = RpfArchive::from_bytes(synth_bytes).unwrap();
        let t0 = Instant::now();
        let b1 = archive.read_raw_by_path("diffuse_tex_01.dds").unwrap();
        let b2 = archive.read_raw_by_path("diffuse_tex_02.dds").unwrap();
        let el = t0.elapsed().as_secs_f64();
        let mb = (b1.len() + b2.len()) as f64 / (1024.0 * 1024.0);
        println!(
            "  Synthetic RPF Stream         : {:>8.2} ms | {:>8.2} MB read | {:>10.1} MB/s throughput",
            el * 1000.0,
            mb,
            mb / el
        );
    }

    // 2. Open-World Progressive Mipmap Streaming Simulation
    // 1,000 textures in a 2000m x 2000m metropolitan area, sports car moving at 120 km/h (33.3 m/s)
    let texture_count = 1_000;
    let mut streamer = TextureStreamer::new(512 * 1024 * 1024); // 512 MB VRAM budget
    streamer.set_bandwidth_limit(64 * 1024 * 1024); // 64 MB / frame transfer cap

    for i in 0..texture_count {
        let x = ((i % 50) as f32 - 25.0) * 40.0;
        let z = ((i / 50) as f32) * 50.0;
        let format = if i % 2 == 0 { TextureFormat::Bc1Dxt1 } else { TextureFormat::Bc7 };
        let tex = StreamableTexture::new(
            TextureId(i as u64),
            format!("asset_tex_{:04}", i),
            2048,
            2048,
            format,
            Vec3::new(x, 0.0, z),
        );
        streamer.register_texture(tex);
    }

    let frames = 300;
    let speed = 33.3; // 120 km/h in m/s
    let dt = 1.0 / 60.0;
    let mut cam_pos = Vec3::new(0.0, 1.8, 0.0);
    let cam_dir = Vec3::new(0.0, 0.0, 1.0);

    let t0 = Instant::now();
    for _ in 0..frames {
        cam_pos.z += speed * dt;
        streamer.update(cam_pos, cam_dir, dt);
    }
    let el = t0.elapsed().as_secs_f64();
    let stats = streamer.stats();

    println!(
        "  {:>4} Textures (512MB Budget) : {:>6.2} µs/frame | {:>5.0} FPS headroom | {:>6.1} MB active VRAM",
        texture_count,
        (el * 1_000_000.0) / frames as f64,
        frames as f64 / el,
        stats.current_vram_used_bytes as f64 / (1024.0 * 1024.0)
    );
    println!(
        "  Progressive Mip Streamed     : {:>6.1} MB total | {:>8.1} MB/s effective throughput | {} evictions",
        stats.cumulative_bytes_streamed as f64 / (1024.0 * 1024.0),
        (stats.cumulative_bytes_streamed as f64 / (1024.0 * 1024.0)) / el,
        stats.mips_evicted_this_frame
    );
}

fn bench_rsc7() {
    bench_section("GTA 5 RSC7 Resource Parsing & 3D Mesh Extraction (ow-asset)");

    let car_bytes = create_synthetic_rsc7_drawable("adder_chassis.ydr", "car");
    let n = 20_000;
    let t0 = Instant::now();
    let mut total_verts = 0;
    let mut total_tris = 0;

    for _ in 0..n {
        if let Ok(drawable) = Rsc7Drawable::from_bytes(&car_bytes) {
            if let Some(lod) = &drawable.lod_high {
                for geom in &lod.submeshes {
                    total_verts += geom.vertex_count as usize;
                    total_tris += geom.index_count as usize / 3;
                }
            }
        }
    }
    let el = t0.elapsed().as_secs_f64();
    let total_bytes_mb = (car_bytes.len() * n) as f64 / (1024.0 * 1024.0);
    let throughput = total_bytes_mb / el;

    println!(
        "  {:>4} RSC7 3D Drawables parsed : {:>8.2} ms | {:>10.0} drawables/sec | {:>7.1} MB/s throughput",
        n,
        el * 1000.0,
        n as f64 / el,
        throughput
    );
    println!(
        "  Extracted Geometry Metrics   : {} vertices | {} triangles verified per model",
        total_verts / n,
        total_tris / n
    );
}

fn bench_rasterizer() {
    bench_section("3D Software Rasterization & Framebuffer Pipeline (ow-render)");

    let width = 640;
    let height = 480;
    let mut fb = FrameBuffer::new(width, height);
    let mut rasterizer = Rasterizer::new();
    rasterizer.shading_model = ShadingModel::Phong;

    let car_bytes = create_synthetic_rsc7_drawable("adder.ydr", "car");
    let drawable = Rsc7Drawable::from_bytes(&car_bytes).unwrap();
    let geom = &drawable.lod_high.as_ref().unwrap().submeshes[0];
    let vertices: Vec<Vertex> = geom.to_vertex_buffer().into_iter().map(|v| {
        Vertex::new(v.position).with_normal(v.normal).with_uv(v.uv0).with_color(v.color)
    }).collect();
    let indices = geom.to_indices();

    let proj = Mat4::perspective(60.0f32.to_radians(), width as f32 / height as f32, 0.1, 100.0);
    let view = Mat4::look_at(Vec3::new(0.0, 2.5, 6.0), Vec3::new(0.0, 0.5, 0.0), Vec3::UP);
    let model = Mat4::identity();
    let light_dir = Vec3::new(0.6, -1.0, 0.4).normalize();
    let light_col = Vec3::new(1.0, 0.95, 0.85);

    let frames = 600;
    let t0 = Instant::now();
    for _ in 0..frames {
        fb.clear(0xFF101820, 1.0);
        rasterizer.draw_mesh(&mut fb, &vertices, &indices, &model, &view, &proj, light_dir, light_col);
    }
    let el = t0.elapsed().as_secs_f64();
    let stats = rasterizer.stats();
    let total_tris = (indices.len() / 3) * frames;
    let frame_ms = (el / frames as f64) * 1000.0;
    let fps = frames as f64 / el;
    let mtris_per_sec = (total_tris as f64 / el) / 1_000_000.0;
    let mpx_per_sec = (stats.fragments_processed as f64 / el) / 1_000_000.0;

    println!(
        "  640x480 Phong 3D Render Loop : {:>6.2} ms/frame | {:>7.0} FPS | {:>6.2} Mtris/sec",
        frame_ms,
        fps,
        mtris_per_sec
    );
    println!(
        "  Rasterizer Pixel Fill Rate   : {:>6.1} Mpixels/sec | {} frags processed | {} depth passed",
        mpx_per_sec,
        stats.fragments_processed,
        stats.fragments_passed
    );
}
