use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    MoveForward,
    MoveBackward,
    MoveLeft,
    MoveRight,
    Jump,
    Crouch,
    Sprint,
    Interact,
    Attack,
    Aim,
    Reload,
    Inventory,
    Map,
    Pause,
    CameraUp,
    CameraDown,
    CameraLeft,
    CameraRight,
    CameraZoomIn,
    CameraZoomOut,
    VehicleAccelerate,
    VehicleBrake,
    VehicleSteerLeft,
    VehicleSteerRight,
    VehicleHorn,
    VehicleExit,
    Custom(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionState {
    Pressed,
    Released,
    Held,
    DoubleTap,
}

#[derive(Debug, Clone)]
pub struct ActionBinding {
    pub action: Action,
    pub key: Key,
    pub modifiers: Modifiers,
    pub dead_zone: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    Escape, Tab, CapsLock, ShiftLeft, ShiftRight,
    ControlLeft, ControlRight, AltLeft, AltRight,
    Space, Enter, Backspace, Delete, Insert,
    Home, End, PageUp, PageDown,
    ArrowUp, ArrowDown, ArrowLeft, ArrowRight,
    MouseLeft, MouseRight, MouseMiddle,
    MouseButton4, MouseButton5,
    GamepadA, GamepadB, GamepadX, GamepadY,
    GamepadLeftShoulder, GamepadRightShoulder,
    GamepadLeftTrigger, GamepadRightTrigger,
    GamepadDPadUp, GamepadDPadDown, GamepadDPadLeft, GamepadDPadRight,
    GamepadLeftStick, GamepadRightStick,
    GamepadStart, GamepadBack,
}

impl Key {
    pub fn key_code(&self) -> u32 {
        *self as u32
    }

    pub fn from_key_code(code: u32) -> Option<Key> {
        use Key::*;
        Some(match code {
            0 => A, 1 => B, 2 => C, 3 => D, 4 => E, 5 => F, 6 => G, 7 => H, 8 => I,
            9 => J, 10 => K, 11 => L, 12 => M, 13 => N, 14 => O, 15 => P, 16 => Q,
            17 => R, 18 => S, 19 => T, 20 => U, 21 => V, 22 => W, 23 => X, 24 => Y,
            25 => Z, 26 => Num0, 27 => Num1, 28 => Num2, 29 => Num3, 30 => Num4,
            31 => Num5, 32 => Num6, 33 => Num7, 34 => Num8, 35 => Num9,
            36 => F1, 37 => F2, 38 => F3, 39 => F4, 40 => F5, 41 => F6, 42 => F7,
            43 => F8, 44 => F9, 45 => F10, 46 => F11, 47 => F12,
            48 => Escape, 49 => Tab, 50 => CapsLock, 51 => ShiftLeft, 52 => ShiftRight,
            53 => ControlLeft, 54 => ControlRight, 55 => AltLeft, 56 => AltRight,
            57 => Space, 58 => Enter, 59 => Backspace, 60 => Delete, 61 => Insert,
            62 => Home, 63 => End, 64 => PageUp, 65 => PageDown,
            66 => ArrowUp, 67 => ArrowDown, 68 => ArrowLeft, 69 => ArrowRight,
            70 => MouseLeft, 71 => MouseRight, 72 => MouseMiddle,
            73 => MouseButton4, 74 => MouseButton5,
            75 => GamepadA, 76 => GamepadB, 77 => GamepadX, 78 => GamepadY,
            79 => GamepadLeftShoulder, 80 => GamepadRightShoulder,
            81 => GamepadLeftTrigger, 82 => GamepadRightTrigger,
            83 => GamepadDPadUp, 84 => GamepadDPadDown, 85 => GamepadDPadLeft, 86 => GamepadDPadRight,
            87 => GamepadLeftStick, 88 => GamepadRightStick,
            89 => GamepadStart, 90 => GamepadBack,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl ActionBinding {
    pub fn new(action: Action, key: Key) -> Self {
        Self {
            action,
            key,
            modifiers: Modifiers::default(),
            dead_zone: 0.0,
        }
    }

    pub fn with_modifiers(mut self, modifiers: Modifiers) -> Self {
        self.modifiers = modifiers;
        self
    }

    pub fn with_dead_zone(mut self, dead_zone: f32) -> Self {
        self.dead_zone = dead_zone;
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct ActionMap {
    bindings: HashMap<u32, Vec<ActionBinding>>,
}

impl ActionMap {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    pub fn bind(&mut self, binding: ActionBinding) {
        self.bindings
            .entry(binding.key.key_code())
            .or_default()
            .push(binding);
    }

    pub fn unbind(&mut self, key_code: u32) {
        self.bindings.remove(&key_code);
    }

    pub fn unbind_key(&mut self, key: Key) {
        self.bindings.remove(&key.key_code());
    }

    pub fn get_bindings(&self, key_code: u32) -> Option<&Vec<ActionBinding>> {
        self.bindings.get(&key_code)
    }

    pub fn binding_count(&self) -> usize {
        self.bindings.values().map(|v| v.len()).sum()
    }

    pub fn clear(&mut self) {
        self.bindings.clear();
    }
}
