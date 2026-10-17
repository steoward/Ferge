use ow_asset::rsc7::{create_synthetic_rsc7_drawable, Rsc7Drawable};
use ow_render::mesh::{Mesh, Vertex};
use ow_render::renderer::{Renderer, RenderConfig};
use ow_render::camera::Camera;
use ow_render::light::{Light, LightType};
use ow_render::rasterizer::{FrameBuffer, Rasterizer, SoftwareRasterPass, ShadingModel};
use ow_render::pass::RenderPass;
use ow_math::{Mat4, Vec3, Vec4};

fn rsc7_to_render_mesh(drawable: &Rsc7Drawable) -> Mesh {
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

    Mesh::new(&drawable.name)
        .with_vertices(all_vertices)
        .with_indices(all_indices)
}

#[test]
fn test_rsc7_drawable_to_render_mesh_conversion() {
    let car_bytes = create_synthetic_rsc7_drawable("adder_chassis.ydr", "car");
    let drawable = Rsc7Drawable::from_bytes(&car_bytes).expect("Failed to parse synthetic RSC7 car");

    assert_eq!(drawable.name, "adder_chassis.ydr");
    assert!(drawable.lod_high.is_some());

    let mesh = rsc7_to_render_mesh(&drawable);
    assert!(mesh.vertex_count() > 0);
    assert!(mesh.triangle_count() > 0);
}

#[test]
fn test_software_rasterizer_full_frame_rendering() {
    let car_bytes = create_synthetic_rsc7_drawable("sports_car.ydr", "car");
    let drawable = Rsc7Drawable::from_bytes(&car_bytes).unwrap();
    let mesh = rsc7_to_render_mesh(&drawable);

    let config = RenderConfig {
        width: 320,
        height: 240,
        clear_color: Vec4::new(0.05, 0.05, 0.1, 1.0),
        ..Default::default()
    };
    let renderer = Renderer::new(config);

    // Setup camera looking at car origin from (0, 3, 7)
    let mut camera = Camera::new_perspective(Vec3::new(0.0, 3.0, 7.0), 60.0f32.to_radians(), 0.1, 100.0);
    camera.view_matrix = Mat4::look_at(Vec3::new(0.0, 3.0, 7.0), Vec3::new(0.0, 0.5, 0.0), Vec3::UP);
    camera.projection_matrix = Mat4::perspective(60.0f32.to_radians(), 320.0 / 240.0, 0.1, 100.0);
    camera.view_projection_matrix = camera.projection_matrix * camera.view_matrix;
    renderer.add_camera(camera);

    // Setup directional sun light
    let light = Light::new("Sun", LightType::Directional)
        .with_direction(Vec3::new(0.5, -1.0, 0.5))
        .with_color(Vec3::new(1.0, 0.95, 0.8));
    renderer.add_light(light);

    // Register 3D car mesh
    let mesh_id = renderer.register_mesh(mesh);
    assert_eq!(mesh_id, 0);

    // Execute SoftwareRasterPass
    let pass = SoftwareRasterPass::new(320, 240);
    pass.execute(&renderer);

    // Inspect framebuffer
    let fb_lock = pass.framebuffer();
    let fb = fb_lock.read();

    let stats = pass.rasterizer().read().stats();
    assert!(stats.triangles_rendered > 0, "At least some triangles must be rendered");
    assert!(stats.fragments_passed > 0, "Visible fragments should pass depth test");

    // Verify depth buffer has non-infinity values where the car was drawn
    let mut drawn_pixels = 0;
    for y in 0..240 {
        for x in 0..320 {
            if fb.get_depth(x, y) < 1.0 {
                drawn_pixels += 1;
            }
        }
    }
    assert!(drawn_pixels > 50, "Expected car to cover at least 50 screen pixels, found {}", drawn_pixels);

    // Test BMP export
    let tmp_path = std::env::temp_dir().join("ferge_test_render.bmp");
    let export_res = fb.save_bmp(&tmp_path);
    assert!(export_res.is_ok());

    let bmp_bytes = std::fs::read(&tmp_path).expect("Read exported test BMP");
    assert_eq!(&bmp_bytes[0..2], b"BM", "BMP magic must be BM");
    let _ = std::fs::remove_file(tmp_path);
}

#[test]
fn test_rasterizer_depth_sorting_and_wireframe() {
    let mut fb = FrameBuffer::new(160, 120);
    fb.clear(0xFF000000, 1.0);

    let mut rasterizer = Rasterizer::new();
    rasterizer.shading_model = ShadingModel::Wireframe;

    let cube_bytes = create_synthetic_rsc7_drawable("cube.ydr", "cube");
    let drawable = Rsc7Drawable::from_bytes(&cube_bytes).unwrap();
    let mesh = rsc7_to_render_mesh(&drawable);

    let proj = Mat4::perspective(60.0f32.to_radians(), 160.0 / 120.0, 0.1, 100.0);
    let view = Mat4::look_at(Vec3::new(0.0, 2.0, 5.0), Vec3::ZERO, Vec3::UP);
    let model = Mat4::identity();

    rasterizer.draw_mesh(
        &mut fb,
        &mesh.vertex_buffer.vertices,
        &mesh.index_buffer.indices,
        &model,
        &view,
        &proj,
        Vec3::new(0.0, -1.0, 0.0),
        Vec3::ONE,
    );

    let stats = rasterizer.stats();
    assert_eq!(stats.triangles_rendered, 12); // 6 faces * 2 triangles = 12
}
