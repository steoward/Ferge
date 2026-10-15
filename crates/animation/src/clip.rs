//! Animation clips: a set of bone tracks plus playback state.
//!
//! A clip owns one track per animated bone, indexed to match the skeleton.
//! Bones without a track simply hold their bind pose.

use crate::track::{Interpolation, TransformTrack};
use ow_math::Transform;

/// One channel of animated bone transforms, indexed by skeleton bone index.
#[derive(Debug, Clone)]
pub struct ClipChannel {
    pub bone: usize,
    pub track: TransformTrack,
}

/// The result of sampling a clip at a specific time.
#[derive(Debug, Clone)]
pub struct ClipSample {
    /// Local-space transform per bone. Bones with no channel keep
    /// `Transform::identity()`.
    pub locals: Vec<Transform>,
    /// The sampled time, in seconds, after looping/clamping.
    pub normalized_time: f32,
}

/// How a clip behaves at the end of its duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WrapMode {
    /// Freeze on the last frame.
    Once,
    /// Jump back to the first frame.
    Loop,
    /// Ping-pong between first and last frame.
    PingPong,
}

/// A source animation: bone tracks plus timing metadata.
#[derive(Debug, Clone)]
pub struct AnimationClip {
    pub name: String,
    pub channels: Vec<ClipChannel>,
    pub duration: f32,
    pub interpolation: Interpolation,
    pub wrap_mode: WrapMode,
    /// Playback speed multiplier.
    pub speed: f32,
}

impl AnimationClip {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            channels: Vec::new(),
            duration: 0.0,
            interpolation: Interpolation::Linear,
            wrap_mode: WrapMode::Loop,
            speed: 1.0,
        }
    }

    /// Adds a channel animating `bone`.
    pub fn add_channel(&mut self, bone: usize, channel_track: TransformTrack) {
        let duration = channel_track.duration();
        self.channels.push(ClipChannel {
            bone,
            track: channel_track,
        });
        self.duration = self.duration.max(duration);
    }

    pub fn with_wrap_mode(mut self, mode: WrapMode) -> Self {
        self.wrap_mode = mode;
        self
    }

    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }

    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    pub fn bone_count(&self) -> usize {
        self.channels.len()
    }

    /// Maps a raw playback time into the clip's time domain, honouring the
    /// wrap mode. Returns the effective time to sample at.
    pub fn map_time(&self, time: f32) -> f32 {
        let d = self.duration;
        if d <= 0.0 {
            return 0.0;
        }
        let t = time * self.speed;
        match self.wrap_mode {
            WrapMode::Once => t.clamp(0.0, d),
            WrapMode::Loop => ((t % d) + d) % d,
            WrapMode::PingPong => {
                let cycle = (t % (2.0 * d)) / d;
                // 0..1 forward, 1..2 backward.
                if cycle <= 1.0 {
                    cycle * d
                } else {
                    (2.0 - cycle) * d
                }
            }
        }
    }

    /// Samples the clip at raw playback `time`, filling an output buffer sized
    /// to `bone_count`.
    pub fn sample(&self, time: f32, bone_count: usize) -> ClipSample {
        let sampled = self.map_time(time);
        let mut locals = vec![Transform::identity(); bone_count];

        for channel in &self.channels {
            if channel.bone < bone_count {
                locals[channel.bone] = channel.track.sample(sampled);
            }
        }

        ClipSample {
            locals,
            normalized_time: if self.duration > 0.0 {
                sampled / self.duration
            } else {
                0.0
            },
        }
    }
}
