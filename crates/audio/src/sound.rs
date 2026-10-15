use ow_math::Vec3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlaybackMode {
    Once,
    Loop,
    PingPong,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundFormat {
    PCM8,
    PCM16,
    PCM24,
    PCM32,
    Float32,
    ADPCM,
    MP3,
    OGG,
    FLAC,
}

#[derive(Debug, Clone)]
pub struct Sound {
    pub name: String,
    pub format: SoundFormat,
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
    pub duration: f32,
    pub state: SoundState,
    pub playback_mode: PlaybackMode,
    pub volume: f32,
    pub pitch: f32,
    pub is_3d: bool,
    pub min_distance: f32,
    pub max_distance: f32,
    pub position: Vec3,
    pub velocity: Vec3,
}

impl Sound {
    pub fn new(name: impl Into<String>, format: SoundFormat, sample_rate: u32, channels: u16) -> Self {
        Self {
            name: name.into(),
            format,
            sample_rate,
            channels,
            samples: Vec::new(),
            duration: 0.0,
            state: SoundState::Stopped,
            playback_mode: PlaybackMode::Once,
            volume: 1.0,
            pitch: 1.0,
            is_3d: false,
            min_distance: 1.0,
            max_distance: 100.0,
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
        }
    }

    pub fn with_samples(mut self, samples: Vec<f32>) -> Self {
        self.duration = samples.len() as f32 / (self.sample_rate as f32 * self.channels as f32);
        self.samples = samples;
        self
    }

    pub fn with_playback_mode(mut self, mode: PlaybackMode) -> Self {
        self.playback_mode = mode;
        self
    }

    pub fn with_volume(mut self, volume: f32) -> Self {
        self.volume = volume.clamp(0.0, 1.0);
        self
    }

    pub fn with_pitch(mut self, pitch: f32) -> Self {
        self.pitch = pitch.max(0.0);
        self
    }

    pub fn with_3d(mut self, is_3d: bool) -> Self {
        self.is_3d = is_3d;
        self
    }

    pub fn with_distance_range(mut self, min: f32, max: f32) -> Self {
        self.min_distance = min;
        self.max_distance = max;
        self
    }

    pub fn with_position(mut self, position: Vec3) -> Self {
        self.position = position;
        self
    }

    pub fn with_velocity(mut self, velocity: Vec3) -> Self {
        self.velocity = velocity;
        self
    }

    pub fn play(&mut self) {
        self.state = SoundState::Playing;
    }

    pub fn pause(&mut self) {
        if self.state == SoundState::Playing {
            self.state = SoundState::Paused;
        }
    }

    pub fn resume(&mut self) {
        if self.state == SoundState::Paused {
            self.state = SoundState::Playing;
        }
    }

    pub fn stop(&mut self) {
        self.state = SoundState::Stopped;
    }

    pub fn is_playing(&self) -> bool {
        self.state == SoundState::Playing
    }

    pub fn is_paused(&self) -> bool {
        self.state == SoundState::Paused
    }

    pub fn is_stopped(&self) -> bool {
        self.state == SoundState::Stopped
    }

    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    pub fn channel_count(&self) -> usize {
        self.channels as usize
    }

    pub fn size_in_bytes(&self) -> usize {
        self.samples.len() * std::mem::size_of::<f32>()
    }
}
