use std::collections::HashMap;
use parking_lot::RwLock;
use crate::action::{Action, ActionBinding, ActionMap, ActionState, Key};
use crate::device::{Gamepad, Keyboard, Mouse};
use ow_math::Vec2;

pub struct InputSystem {
    keyboards: RwLock<Vec<Keyboard>>,
    mice: RwLock<Vec<Mouse>>,
    gamepads: RwLock<Vec<Gamepad>>,
    action_map: RwLock<ActionMap>,
    action_states: RwLock<HashMap<Action, ActionState>>,
    text_input: RwLock<String>,
}

impl InputSystem {
    pub fn new() -> Self {
        Self {
            keyboards: RwLock::new(Vec::new()),
            mice: RwLock::new(Vec::new()),
            gamepads: RwLock::new(Vec::new()),
            action_map: RwLock::new(ActionMap::new()),
            action_states: RwLock::new(HashMap::new()),
            text_input: RwLock::new(String::new()),
        }
    }

    pub fn add_keyboard(&self, keyboard: Keyboard) {
        self.keyboards.write().push(keyboard);
    }

    pub fn add_mouse(&self, mouse: Mouse) {
        self.mice.write().push(mouse);
    }

    pub fn add_gamepad(&self, gamepad: Gamepad) {
        self.gamepads.write().push(gamepad);
    }

    pub fn bind_action(&self, binding: ActionBinding) {
        self.action_map.write().bind(binding);
    }

    pub fn unbind_action(&self, key: Key) {
        self.action_map.write().unbind_key(key);
    }

    pub fn is_action_pressed(&self, action: Action) -> bool {
        self.action_states
            .read()
            .get(&action)
            .map(|s| *s == ActionState::Pressed || *s == ActionState::Held)
            .unwrap_or(false)
    }

    pub fn is_action_just_pressed(&self, action: Action) -> bool {
        self.action_states
            .read()
            .get(&action)
            .map(|s| *s == ActionState::Pressed)
            .unwrap_or(false)
    }

    pub fn is_action_just_released(&self, action: Action) -> bool {
        self.action_states
            .read()
            .get(&action)
            .map(|s| *s == ActionState::Released)
            .unwrap_or(false)
    }

    pub fn get_mouse_position(&self) -> Vec2 {
        self.mice
            .read()
            .first()
            .map(|m| m.position)
            .unwrap_or(Vec2::ZERO)
    }

    pub fn get_mouse_delta(&self) -> Vec2 {
        self.mice
            .read()
            .first()
            .map(|m| m.delta)
            .unwrap_or(Vec2::ZERO)
    }

    pub fn get_mouse_wheel(&self) -> f32 {
        self.mice
            .read()
            .first()
            .map(|m| m.wheel)
            .unwrap_or(0.0)
    }

    pub fn is_mouse_button_pressed(&self, button: u8) -> bool {
        self.mice
            .read()
            .first()
            .map(|m| m.is_pressed(button))
            .unwrap_or(false)
    }

    pub fn is_mouse_button_just_pressed(&self, button: u8) -> bool {
        self.mice
            .read()
            .first()
            .map(|m| m.is_just_pressed(button))
            .unwrap_or(false)
    }

    pub fn get_gamepad_left_stick(&self, gamepad_id: u32) -> Vec2 {
        self.gamepads
            .read()
            .iter()
            .find(|g| g.device.id == gamepad_id)
            .map(|g| g.get_left_stick())
            .unwrap_or(Vec2::ZERO)
    }

    pub fn get_gamepad_right_stick(&self, gamepad_id: u32) -> Vec2 {
        self.gamepads
            .read()
            .iter()
            .find(|g| g.device.id == gamepad_id)
            .map(|g| g.get_right_stick())
            .unwrap_or(Vec2::ZERO)
    }

    pub fn get_gamepad_trigger(&self, gamepad_id: u32, right: bool) -> f32 {
        self.gamepads
            .read()
            .iter()
            .find(|g| g.device.id == gamepad_id)
            .map(|g| if right { g.right_trigger } else { g.left_trigger })
            .unwrap_or(0.0)
    }

    pub fn is_gamepad_button_pressed(&self, gamepad_id: u32, button: u8) -> bool {
        self.gamepads
            .read()
            .iter()
            .find(|g| g.device.id == gamepad_id)
            .map(|g| g.is_pressed(button))
            .unwrap_or(false)
    }

    pub fn is_gamepad_button_just_pressed(&self, gamepad_id: u32, button: u8) -> bool {
        self.gamepads
            .read()
            .iter()
            .find(|g| g.device.id == gamepad_id)
            .map(|g| g.is_just_pressed(button))
            .unwrap_or(false)
    }

    pub fn set_text_input(&self, text: impl Into<String>) {
        *self.text_input.write() = text.into();
    }

    pub fn get_text_input(&self) -> String {
        self.text_input.read().clone()
    }

    pub fn clear_text_input(&self) {
        self.text_input.write().clear();
    }

    pub fn update(&self) {
        for keyboard in self.keyboards.write().iter_mut() {
            keyboard.update();
        }
        for mouse in self.mice.write().iter_mut() {
            mouse.update();
        }
        for gamepad in self.gamepads.write().iter_mut() {
            gamepad.update();
        }

        let mut action_states = self.action_states.write();
        action_states.clear();

        let action_map = self.action_map.read();
        for keyboard in self.keyboards.read().iter() {
            for key_code in 0..keyboard.keys.len() {
                let pressed = keyboard.keys[key_code];
                if let Some(bindings) = action_map.get_bindings(key_code as u32) {
                    for binding in bindings {
                        let state = if pressed {
                            if keyboard.is_just_pressed(key_code as u8) {
                                ActionState::Pressed
                            } else {
                                ActionState::Held
                            }
                        } else if keyboard.is_just_released(key_code as u8) {
                            ActionState::Released
                        } else {
                            continue;
                        };
                        action_states.insert(binding.action, state);
                    }
                }
            }
        }
    }
}

impl Default for InputSystem {
    fn default() -> Self {
        Self::new()
    }
}
