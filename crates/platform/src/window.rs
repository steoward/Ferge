use parking_lot::RwLock;

#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub resizable: bool,
    pub fullscreen: bool,
    pub borderless: bool,
    pub maximized: bool,
    pub minimized: bool,
    pub visible: bool,
    pub focused: bool,
    pub always_on_top: bool,
    pub vsync: bool,
    pub msaa_samples: u32,
    pub depth_bits: u8,
    pub stencil_bits: u8,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Open World Engine".to_string(),
            width: 1920,
            height: 1080,
            x: 100,
            y: 100,
            resizable: true,
            fullscreen: false,
            borderless: false,
            maximized: false,
            minimized: false,
            visible: true,
            focused: true,
            always_on_top: false,
            vsync: true,
            msaa_samples: 4,
            depth_bits: 24,
            stencil_bits: 8,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum WindowEvent {
    Resized { width: u32, height: u32 },
    Moved { x: i32, y: i32 },
    Focused,
    Unfocused,
    Minimized,
    Maximized,
    Restored,
    CloseRequested,
    KeyPressed { key: u32, scancode: u32 },
    KeyReleased { key: u32, scancode: u32 },
    MouseButtonPressed { button: u32 },
    MouseButtonReleased { button: u32 },
    MouseMoved { x: f32, y: f32 },
    MouseWheel { delta: f32 },
    FileDropped { path: String },
}

pub struct Window {
    config: RwLock<WindowConfig>,
    events: RwLock<Vec<WindowEvent>>,
    handle: *mut std::ffi::c_void,
    is_open: RwLock<bool>,
}

impl Window {
    pub fn new(config: WindowConfig) -> Self {
        Self {
            config: RwLock::new(config),
            events: RwLock::new(Vec::new()),
            handle: std::ptr::null_mut(),
            is_open: RwLock::new(true),
        }
    }

    pub fn get_config(&self) -> WindowConfig {
        self.config.read().clone()
    }

    pub fn set_config(&self, config: WindowConfig) {
        *self.config.write() = config;
    }

    pub fn push_event(&self, event: WindowEvent) {
        self.events.write().push(event);
    }

    pub fn poll_events(&self) -> Vec<WindowEvent> {
        std::mem::take(&mut *self.events.write())
    }

    pub fn is_open(&self) -> bool {
        *self.is_open.read()
    }

    pub fn close(&self) {
        *self.is_open.write() = false;
    }

    pub fn set_title(&self, title: impl Into<String>) {
        self.config.write().title = title.into();
    }

    pub fn set_size(&self, width: u32, height: u32) {
        let mut config = self.config.write();
        config.width = width;
        config.height = height;
    }

    pub fn get_size(&self) -> (u32, u32) {
        let config = self.config.read();
        (config.width, config.height)
    }

    pub fn set_position(&self, x: i32, y: i32) {
        let mut config = self.config.write();
        config.x = x;
        config.y = y;
    }

    pub fn get_position(&self) -> (i32, i32) {
        let config = self.config.read();
        (config.x, config.y)
    }

    pub fn set_fullscreen(&self, fullscreen: bool) {
        self.config.write().fullscreen = fullscreen;
    }

    pub fn is_fullscreen(&self) -> bool {
        self.config.read().fullscreen
    }

    pub fn set_vsync(&self, vsync: bool) {
        self.config.write().vsync = vsync;
    }

    pub fn is_vsync(&self) -> bool {
        self.config.read().vsync
    }

    pub fn native_handle(&self) -> *mut std::ffi::c_void {
        self.handle
    }
}
