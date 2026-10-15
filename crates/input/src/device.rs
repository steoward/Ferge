use ow_math::Vec2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceType {
    Keyboard,
    Mouse,
    Gamepad,
    Touch,
    VRController,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub device_type: DeviceType,
    pub id: u32,
    pub name: String,
}

impl Device {
    pub fn new(device_type: DeviceType, id: u32, name: impl Into<String>) -> Self {
        Self {
            device_type,
            id,
            name: name.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Keyboard {
    pub device: Device,
    pub keys: [bool; 256],
    pub prev_keys: [bool; 256],
}

impl Keyboard {
    pub fn new(id: u32) -> Self {
        Self {
            device: Device::new(DeviceType::Keyboard, id, "Keyboard"),
            keys: [false; 256],
            prev_keys: [false; 256],
        }
    }

    pub fn is_pressed(&self, key: u8) -> bool {
        self.keys[key as usize]
    }

    pub fn is_just_pressed(&self, key: u8) -> bool {
        self.keys[key as usize] && !self.prev_keys[key as usize]
    }

    pub fn is_just_released(&self, key: u8) -> bool {
        !self.keys[key as usize] && self.prev_keys[key as usize]
    }

    pub fn update(&mut self) {
        self.prev_keys = self.keys;
    }

    pub fn set_key(&mut self, key: u8, pressed: bool) {
        self.keys[key as usize] = pressed;
    }
}

#[derive(Debug, Clone)]
pub struct Mouse {
    pub device: Device,
    pub position: Vec2,
    pub prev_position: Vec2,
    pub delta: Vec2,
    pub buttons: [bool; 8],
    pub prev_buttons: [bool; 8],
    pub wheel: f32,
    pub prev_wheel: f32,
}

impl Mouse {
    pub fn new(id: u32) -> Self {
        Self {
            device: Device::new(DeviceType::Mouse, id, "Mouse"),
            position: Vec2::ZERO,
            prev_position: Vec2::ZERO,
            delta: Vec2::ZERO,
            buttons: [false; 8],
            prev_buttons: [false; 8],
            wheel: 0.0,
            prev_wheel: 0.0,
        }
    }

    pub fn is_pressed(&self, button: u8) -> bool {
        self.buttons[button as usize]
    }

    pub fn is_just_pressed(&self, button: u8) -> bool {
        self.buttons[button as usize] && !self.prev_buttons[button as usize]
    }

    pub fn is_just_released(&self, button: u8) -> bool {
        !self.buttons[button as usize] && self.prev_buttons[button as usize]
    }

    pub fn update(&mut self) {
        self.prev_position = self.position;
        self.delta = self.position - self.prev_position;
        self.prev_buttons = self.buttons;
        self.prev_wheel = self.wheel;
        self.wheel = 0.0;
    }

    pub fn set_position(&mut self, x: f32, y: f32) {
        self.position = Vec2::new(x, y);
    }

    pub fn set_button(&mut self, button: u8, pressed: bool) {
        self.buttons[button as usize] = pressed;
    }

    pub fn add_wheel(&mut self, delta: f32) {
        self.wheel += delta;
    }
}

#[derive(Debug, Clone)]
pub struct Gamepad {
    pub device: Device,
    pub buttons: [bool; 16],
    pub prev_buttons: [bool; 16],
    pub left_stick: Vec2,
    pub right_stick: Vec2,
    pub left_trigger: f32,
    pub right_trigger: f32,
    pub left_stick_dead_zone: f32,
    pub right_stick_dead_zone: f32,
}

impl Gamepad {
    pub fn new(id: u32) -> Self {
        Self {
            device: Device::new(DeviceType::Gamepad, id, format!("Gamepad {}", id)),
            buttons: [false; 16],
            prev_buttons: [false; 16],
            left_stick: Vec2::ZERO,
            right_stick: Vec2::ZERO,
            left_trigger: 0.0,
            right_trigger: 0.0,
            left_stick_dead_zone: 0.15,
            right_stick_dead_zone: 0.15,
        }
    }

    pub fn is_pressed(&self, button: u8) -> bool {
        self.buttons[button as usize]
    }

    pub fn is_just_pressed(&self, button: u8) -> bool {
        self.buttons[button as usize] && !self.prev_buttons[button as usize]
    }

    pub fn is_just_released(&self, button: u8) -> bool {
        !self.buttons[button as usize] && self.prev_buttons[button as usize]
    }

    pub fn get_left_stick(&self) -> Vec2 {
        if self.left_stick.length() < self.left_stick_dead_zone {
            Vec2::ZERO
        } else {
            self.left_stick
        }
    }

    pub fn get_right_stick(&self) -> Vec2 {
        if self.right_stick.length() < self.right_stick_dead_zone {
            Vec2::ZERO
        } else {
            self.right_stick
        }
    }

    pub fn update(&mut self) {
        self.prev_buttons = self.buttons;
    }

    pub fn set_button(&mut self, button: u8, pressed: bool) {
        self.buttons[button as usize] = pressed;
    }

    pub fn set_left_stick(&mut self, x: f32, y: f32) {
        self.left_stick = Vec2::new(x, y);
    }

    pub fn set_right_stick(&mut self, x: f32, y: f32) {
        self.right_stick = Vec2::new(x, y);
    }

    pub fn set_left_trigger(&mut self, value: f32) {
        self.left_trigger = value.clamp(0.0, 1.0);
    }

    pub fn set_right_trigger(&mut self, value: f32) {
        self.right_trigger = value.clamp(0.0, 1.0);
    }
}
