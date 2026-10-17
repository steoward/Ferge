use std::collections::HashMap;
use parking_lot::RwLock;
use crate::camera::Camera;
use crate::mesh::Mesh;
use crate::material::Material;
use crate::light::Light;
use crate::pass::RenderPass;

#[derive(Debug, Clone)]
pub struct RenderConfig {
    pub width: u32,
    pub height: u32,
    pub vsync: bool,
    pub fullscreen: bool,
    pub multisample_count: u32,
    pub shadow_map_size: u32,
    pub max_lights: usize,
    pub max_draw_calls: usize,
    pub clear_color: ow_math::Vec4,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            vsync: true,
            fullscreen: false,
            multisample_count: 4,
            shadow_map_size: 2048,
            max_lights: 256,
            max_draw_calls: 10000,
            clear_color: ow_math::Vec4::new(0.0, 0.0, 0.0, 1.0),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RenderStats {
    pub draw_calls: u32,
    pub triangles: u32,
    pub vertices: u32,
    pub textures: u32,
    pub shaders: u32,
    pub frame_time_ms: f32,
    pub fps: f32,
}

pub struct Renderer {
    config: RenderConfig,
    cameras: RwLock<Vec<Camera>>,
    meshes: RwLock<HashMap<u32, Mesh>>,
    materials: RwLock<HashMap<u32, Material>>,
    lights: RwLock<Vec<Light>>,
    render_passes: RwLock<Vec<Box<dyn RenderPass>>>,
    stats: RwLock<RenderStats>,
    next_mesh_id: RwLock<u32>,
    next_material_id: RwLock<u32>,
}

impl Renderer {
    pub fn new(config: RenderConfig) -> Self {
        Self {
            config,
            cameras: RwLock::new(Vec::new()),
            meshes: RwLock::new(HashMap::new()),
            materials: RwLock::new(HashMap::new()),
            lights: RwLock::new(Vec::new()),
            render_passes: RwLock::new(Vec::new()),
            stats: RwLock::new(RenderStats::default()),
            next_mesh_id: RwLock::new(0),
            next_material_id: RwLock::new(0),
        }
    }

    pub fn add_camera(&self, camera: Camera) {
        self.cameras.write().push(camera);
    }

    pub fn remove_camera(&self, index: usize) {
        self.cameras.write().remove(index);
    }

    pub fn get_camera(&self, index: usize) -> Option<Camera> {
        self.cameras.read().get(index).cloned()
    }

    pub fn register_mesh(&self, mesh: Mesh) -> u32 {
        let mut next_id = self.next_mesh_id.write();
        let id = *next_id;
        *next_id += 1;
        self.meshes.write().insert(id, mesh);
        id
    }

    pub fn unregister_mesh(&self, id: u32) {
        self.meshes.write().remove(&id);
    }

    pub fn get_mesh(&self, id: u32) -> Option<Mesh> {
        self.meshes.read().get(&id).cloned()
    }

    pub fn get_all_meshes(&self) -> Vec<Mesh> {
        self.meshes.read().values().cloned().collect()
    }

    pub fn register_material(&self, material: Material) -> u32 {
        let mut next_id = self.next_material_id.write();
        let id = *next_id;
        *next_id += 1;
        self.materials.write().insert(id, material);
        id
    }

    pub fn unregister_material(&self, id: u32) {
        self.materials.write().remove(&id);
    }

    pub fn get_material(&self, id: u32) -> Option<Material> {
        self.materials.read().get(&id).cloned()
    }

    pub fn add_light(&self, light: Light) {
        self.lights.write().push(light);
    }

    pub fn remove_light(&self, index: usize) {
        self.lights.write().remove(index);
    }

    pub fn get_light(&self, index: usize) -> Option<Light> {
        self.lights.read().get(index).cloned()
    }

    pub fn get_all_lights(&self) -> Vec<Light> {
        self.lights.read().clone()
    }

    pub fn add_render_pass(&self, pass: Box<dyn RenderPass>) {
        self.render_passes.write().push(pass);
    }

    pub fn remove_render_pass(&self, index: usize) {
        self.render_passes.write().remove(index);
    }

    pub fn render(&self) {
        let mut stats = self.stats.write();
        stats.draw_calls = 0;
        stats.triangles = 0;
        stats.vertices = 0;

        for pass in self.render_passes.read().iter() {
            pass.execute(self);
        }
    }

    pub fn get_stats(&self) -> RenderStats {
        self.stats.read().clone()
    }

    pub fn get_config(&self) -> &RenderConfig {
        &self.config
    }

    pub fn set_config(&mut self, config: RenderConfig) {
        self.config = config;
    }

    pub fn camera_count(&self) -> usize {
        self.cameras.read().len()
    }

    pub fn mesh_count(&self) -> usize {
        self.meshes.read().len()
    }

    pub fn material_count(&self) -> usize {
        self.materials.read().len()
    }

    pub fn light_count(&self) -> usize {
        self.lights.read().len()
    }

    pub fn render_pass_count(&self) -> usize {
        self.render_passes.read().len()
    }
}
