use std::collections::HashMap;
use parking_lot::RwLock;
use crate::sound::Sound;
use crate::listener::AudioListener;

#[derive(Debug, Clone)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_size: u32,
    pub max_voices: usize,
    pub master_volume: f32,
    pub doppler_factor: f32,
    pub speed_of_sound: f32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            buffer_size: 512,
            max_voices: 256,
            master_volume: 1.0,
            doppler_factor: 1.0,
            speed_of_sound: 343.3,
        }
    }
}

pub struct AudioSystem {
    config: AudioConfig,
    sounds: RwLock<HashMap<u32, Sound>>,
    listener: RwLock<AudioListener>,
    next_sound_id: RwLock<u32>,
    is_initialized: RwLock<bool>,
}

impl AudioSystem {
    pub fn new(config: AudioConfig) -> Self {
        Self {
            config,
            sounds: RwLock::new(HashMap::new()),
            listener: RwLock::new(AudioListener::new()),
            next_sound_id: RwLock::new(0),
            is_initialized: RwLock::new(false),
        }
    }

    pub fn initialize(&self) -> bool {
        *self.is_initialized.write() = true;
        true
    }

    pub fn shutdown(&self) {
        *self.is_initialized.write() = false;
    }

    pub fn is_initialized(&self) -> bool {
        *self.is_initialized.read()
    }

    pub fn register_sound(&self, sound: Sound) -> u32 {
        let mut next_id = self.next_sound_id.write();
        let id = *next_id;
        *next_id += 1;
        self.sounds.write().insert(id, sound);
        id
    }

    pub fn unregister_sound(&self, id: u32) {
        self.sounds.write().remove(&id);
    }

    pub fn get_sound(&self, id: u32) -> Option<Sound> {
        self.sounds.read().get(&id).cloned()
    }

    pub fn play_sound(&self, id: u32, _position: Option<ow_math::Vec3>) -> bool {
        let sounds = self.sounds.read();
        if let Some(sound) = sounds.get(&id) {
            let _ = sound;
            true
        } else {
            false
        }
    }

    pub fn stop_sound(&self, _id: u32) {
        // Stop sound implementation
    }

    pub fn pause_sound(&self, _id: u32) {
        // Pause sound implementation
    }

    pub fn resume_sound(&self, _id: u32) {
        // Resume sound implementation
    }

    pub fn set_sound_volume(&self, _id: u32, _volume: f32) {
        // Set volume implementation
    }

    pub fn set_sound_pitch(&self, _id: u32, _pitch: f32) {
        // Set pitch implementation
    }

    pub fn set_listener_position(&self, position: ow_math::Vec3) {
        self.listener.write().position = position;
    }

    pub fn set_listener_orientation(&self, forward: ow_math::Vec3, up: ow_math::Vec3) {
        let mut listener = self.listener.write();
        listener.forward = forward.normalize();
        listener.up = up.normalize();
    }

    pub fn set_master_volume(&self, _volume: f32) {
        // Set master volume implementation
    }

    pub fn get_listener(&self) -> AudioListener {
        self.listener.read().clone()
    }

    pub fn sound_count(&self) -> usize {
        self.sounds.read().len()
    }

    pub fn config(&self) -> &AudioConfig {
        &self.config
    }
}

impl Default for AudioSystem {
    fn default() -> Self {
        Self::new(AudioConfig::default())
    }
}
