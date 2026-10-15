use std::sync::Arc;
use parking_lot::RwLock;
use crate::event::EventBus;
use crate::scene::Scene;
use crate::resource::ResourceManager;

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub window_width: u32,
    pub window_height: u32,
    pub window_title: String,
    pub vsync: bool,
    pub target_fps: u32,
    pub enable_physics: bool,
    pub enable_audio: bool,
    pub enable_networking: bool,
    pub log_level: LogLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            window_width: 1920,
            window_height: 1080,
            window_title: "Open World Engine".to_string(),
            vsync: true,
            target_fps: 60,
            enable_physics: true,
            enable_audio: true,
            enable_networking: false,
            log_level: LogLevel::Info,
        }
    }
}

pub struct Engine {
    config: EngineConfig,
    is_running: RwLock<bool>,
    event_bus: Arc<EventBus>,
    scene: Arc<RwLock<Scene>>,
    resource_manager: Arc<ResourceManager>,
    frame_count: RwLock<u64>,
    delta_time: RwLock<f32>,
    elapsed_time: RwLock<f32>,
}

impl Engine {
    pub fn new(config: EngineConfig) -> Self {
        Self {
            config,
            is_running: RwLock::new(false),
            event_bus: Arc::new(EventBus::new()),
            scene: Arc::new(RwLock::new(Scene::new("MainScene"))),
            resource_manager: Arc::new(ResourceManager::new()),
            frame_count: RwLock::new(0),
            delta_time: RwLock::new(0.0),
            elapsed_time: RwLock::new(0.0),
        }
    }

    pub fn initialize(&self) -> bool {
        *self.is_running.write() = true;
        true
    }

    pub fn shutdown(&self) {
        *self.is_running.write() = false;
    }

    pub fn is_running(&self) -> bool {
        *self.is_running.read()
    }

    pub fn run(&self) {
        if !self.initialize() {
            return;
        }

        while self.is_running() {
            let dt = self.delta_time.read().clamp(0.001, 0.1);
            self.update(dt);
            self.render();
            *self.frame_count.write() += 1;
            *self.elapsed_time.write() += dt;
        }

        self.shutdown();
    }

    pub fn update(&self, dt: f32) {
        *self.delta_time.write() = dt;
        *self.frame_count.write() += 1;
        self.event_bus.process_events();
    }

    pub fn render(&self) {
        // Rendering handled by render system
    }

    pub fn get_config(&self) -> &EngineConfig {
        &self.config
    }

    pub fn get_event_bus(&self) -> Arc<EventBus> {
        Arc::clone(&self.event_bus)
    }

    pub fn get_scene(&self) -> Arc<RwLock<Scene>> {
        Arc::clone(&self.scene)
    }

    pub fn get_resource_manager(&self) -> Arc<ResourceManager> {
        Arc::clone(&self.resource_manager)
    }

    pub fn get_frame_count(&self) -> u64 {
        *self.frame_count.read()
    }

    pub fn get_delta_time(&self) -> f32 {
        *self.delta_time.read()
    }

    pub fn get_elapsed_time(&self) -> f32 {
        *self.elapsed_time.read()
    }

    pub fn get_fps(&self) -> f32 {
        let dt = *self.delta_time.read();
        if dt > 0.0 {
            1.0 / dt
        } else {
            0.0
        }
    }
}
