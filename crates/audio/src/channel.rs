use parking_lot::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelGroup {
    Master,
    Music,
    SFX,
    Voice,
    Ambient,
    UI,
}

#[derive(Debug, Clone)]
pub struct AudioChannel {
    pub name: String,
    pub group: ChannelGroup,
    pub volume: f32,
    pub pitch: f32,
    pub is_muted: bool,
    pub is_solo: bool,
    pub pan: f32,
}

impl AudioChannel {
    pub fn new(name: impl Into<String>, group: ChannelGroup) -> Self {
        Self {
            name: name.into(),
            group,
            volume: 1.0,
            pitch: 1.0,
            is_muted: false,
            is_solo: false,
            pan: 0.0,
        }
    }

    pub fn with_volume(mut self, volume: f32) -> Self {
        self.volume = volume.clamp(0.0, 1.0);
        self
    }

    pub fn with_pitch(mut self, pitch: f32) -> Self {
        self.pitch = pitch.max(0.0);
        self
    }

    pub fn with_muted(mut self, muted: bool) -> Self {
        self.is_muted = muted;
        self
    }

    pub fn with_solo(mut self, solo: bool) -> Self {
        self.is_solo = solo;
        self
    }

    pub fn with_pan(mut self, pan: f32) -> Self {
        self.pan = pan.clamp(-1.0, 1.0);
        self
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    pub fn set_pitch(&mut self, pitch: f32) {
        self.pitch = pitch.max(0.0);
    }

    pub fn mute(&mut self) {
        self.is_muted = true;
    }

    pub fn unmute(&mut self) {
        self.is_muted = false;
    }

    pub fn solo(&mut self) {
        self.is_solo = true;
    }

    pub fn unsolo(&mut self) {
        self.is_solo = false;
    }

    pub fn is_audible(&self) -> bool {
        !self.is_muted && self.volume > 0.0
    }

    pub fn get_effective_volume(&self) -> f32 {
        if self.is_muted {
            0.0
        } else {
            self.volume
        }
    }
}

pub struct ChannelManager {
    channels: RwLock<Vec<AudioChannel>>,
}

impl ChannelManager {
    pub fn new() -> Self {
        Self {
            channels: RwLock::new(Vec::new()),
        }
    }

    pub fn add_channel(&self, channel: AudioChannel) {
        self.channels.write().push(channel);
    }

    pub fn remove_channel(&self, index: usize) {
        self.channels.write().remove(index);
    }

    pub fn get_channel(&self, index: usize) -> Option<AudioChannel> {
        self.channels.read().get(index).cloned()
    }

    pub fn get_channels_by_group(&self, group: ChannelGroup) -> Vec<AudioChannel> {
        self.channels
            .read()
            .iter()
            .filter(|c| c.group == group)
            .cloned()
            .collect()
    }

    pub fn set_group_volume(&self, group: ChannelGroup, volume: f32) {
        for channel in self.channels.write().iter_mut() {
            if channel.group == group {
                channel.set_volume(volume);
            }
        }
    }

    pub fn mute_group(&self, group: ChannelGroup) {
        for channel in self.channels.write().iter_mut() {
            if channel.group == group {
                channel.mute();
            }
        }
    }

    pub fn unmute_group(&self, group: ChannelGroup) {
        for channel in self.channels.write().iter_mut() {
            if channel.group == group {
                channel.unmute();
            }
        }
    }

    pub fn channel_count(&self) -> usize {
        self.channels.read().len()
    }
}

impl Default for ChannelManager {
    fn default() -> Self {
        Self::new()
    }
}
