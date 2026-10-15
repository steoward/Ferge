//! Keyframe tracks and interpolation.
//!
//! Each bone in an engine typically owns three tracks (translation, rotation,
//! scale). Interpolation is chosen per track: step for events, linear for
//! position, and slerp for rotation — the standard split that keeps
//! quaternions on the unit sphere.

use ow_math::{Vec3, Quat, Transform};

/// How values between keyframes are interpolated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Interpolation {
    /// Hold the previous key until the next one (events, visibility).
    Step,
    /// Linear interpolation.
    Linear,
    /// Cubic Hermite spline: slower, far smoother for dense key data.
    Cubic,
}

/// A keyframe with in/out tangents for cubic interpolation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Keyframe<T> {
    pub time: f32,
    pub value: T,
    pub in_tangent: T,
    pub out_tangent: T,
}

impl<T: Default + Copy> Keyframe<T> {
    pub fn new(time: f32, value: T) -> Self {
        Self {
            time,
            value,
            in_tangent: T::default(),
            out_tangent: T::default(),
        }
    }

    pub fn with_tangents(mut self, in_tangent: T, out_tangent: T) -> Self {
        self.in_tangent = in_tangent;
        self.out_tangent = out_tangent;
        self
    }
}

/// Index of the keyframe at or before `time`, clamped to the track range.
/// Returns `None` for an empty track.
fn find_segment(times: &[f32], time: f32) -> Option<usize> {
    if times.is_empty() {
        return None;
    }
    if time <= times[0] {
        return Some(0);
    }
    if time >= times[times.len() - 1] {
        return Some(times.len() - 1);
    }
    match times.binary_search_by(|t| t.total_cmp(&time)) {
        Ok(i) => Some(i),
        Err(i) => Some(i - 1),
    }
}

/// Cubic Hermite basis evaluation: given endpoint values `p0`/`p1` and
/// tangents `m0`/`m1`, returns the interpolated point at `t ∈ [0, 1]`.
fn hermite(p0: f32, p1: f32, m0: f32, m1: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;
    h00 * p0 + h10 * m0 + h01 * p1 + h11 * m1
}

fn hermite_vec3(p0: Vec3, p1: Vec3, m0: Vec3, m1: Vec3, t: f32) -> Vec3 {
    let t2 = t * t;
    let t3 = t2 * t;
    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;
    p0 * h00 + m0 * h10 + p1 * h01 + m1 * h11
}

/// Sorted-key helper shared by every track type.
fn normalize_parameter(time: f32, times: &[f32], i: usize) -> Option<f32> {
    if i + 1 >= times.len() {
        return None;
    }
    let a = times[i];
    let b = times[i + 1];
    Some(if b > a { ((time - a) / (b - a)).clamp(0.0, 1.0) } else { 0.0 })
}

/// Scalar keyframe track.
#[derive(Debug, Clone)]
pub struct ScalarTrack {
    pub keys: Vec<Keyframe<f32>>,
    pub interpolation: Interpolation,
}

impl ScalarTrack {
    pub fn new(interpolation: Interpolation) -> Self {
        Self {
            keys: Vec::new(),
            interpolation,
        }
    }

    pub fn with_keys(mut self, keys: Vec<Keyframe<f32>>) -> Self {
        self.keys = keys;
        self.keys.sort_by(|a, b| a.time.total_cmp(&b.time));
        self
    }

    pub fn add(&mut self, key: Keyframe<f32>) {
        let pos = self
            .keys
            .binary_search_by(|k| k.time.total_cmp(&key.time))
            .unwrap_or_else(|i| i);
        self.keys.insert(pos, key);
    }

    pub fn duration(&self) -> f32 {
        self.keys.last().map(|k| k.time).unwrap_or(0.0)
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn sample(&self, time: f32) -> f32 {
        let times: Vec<f32> = self.keys.iter().map(|k| k.time).collect();
        let i = match find_segment(&times, time) {
            Some(i) => i,
            None => return 0.0,
        };
        let t = match normalize_parameter(time, &times, i) {
            Some(t) => t,
            None => return self.keys[self.keys.len() - 1].value,
        };

        let a = &self.keys[i];
        let b = &self.keys[i + 1];
        match self.interpolation {
            Interpolation::Step => a.value,
            Interpolation::Linear => a.value + (b.value - a.value) * t,
            Interpolation::Cubic => {
                let span = times[i + 1] - times[i];
                hermite(a.value, b.value, a.out_tangent * span, b.in_tangent * span, t)
            }
        }
    }
}

/// Vector keyframe track.
#[derive(Debug, Clone)]
pub struct VectorTrack {
    pub keys: Vec<Keyframe<Vec3>>,
    pub interpolation: Interpolation,
}

impl VectorTrack {
    pub fn new(interpolation: Interpolation) -> Self {
        Self {
            keys: Vec::new(),
            interpolation,
        }
    }

    /// A constant track, useful for scale that never animates.
    pub fn constant(value: Vec3) -> Self {
        Self::new(Interpolation::Step).with_keys(vec![Keyframe::new(0.0, value)])
    }

    pub fn with_keys(mut self, keys: Vec<Keyframe<Vec3>>) -> Self {
        self.keys = keys;
        self.keys.sort_by(|a, b| a.time.total_cmp(&b.time));
        self
    }

    pub fn add(&mut self, key: Keyframe<Vec3>) {
        let pos = self
            .keys
            .binary_search_by(|k| k.time.total_cmp(&key.time))
            .unwrap_or_else(|i| i);
        self.keys.insert(pos, key);
    }

    pub fn duration(&self) -> f32 {
        self.keys.last().map(|k| k.time).unwrap_or(0.0)
    }

    pub fn sample(&self, time: f32) -> Vec3 {
        let times: Vec<f32> = self.keys.iter().map(|k| k.time).collect();
        let i = match find_segment(&times, time) {
            Some(i) => i,
            None => return Vec3::ZERO,
        };
        let t = match normalize_parameter(time, &times, i) {
            Some(t) => t,
            None => return self.keys[self.keys.len() - 1].value,
        };

        let a = &self.keys[i];
        let b = &self.keys[i + 1];
        match self.interpolation {
            Interpolation::Step => a.value,
            Interpolation::Linear => a.value.lerp(&b.value, t),
            Interpolation::Cubic => {
                let span = times[i + 1] - times[i];
                hermite_vec3(a.value, b.value, a.out_tangent * span, b.in_tangent * span, t)
            }
        }
    }
}

/// Quaternion keyframe track. Interpolating quaternions must use slerp; lerp
/// followed by renormalisation shrinks the rotation on fast spins.
#[derive(Debug, Clone)]
pub struct QuaternionTrack {
    pub keys: Vec<Keyframe<Quat>>,
    pub interpolation: Interpolation,
}

impl QuaternionTrack {
    pub fn new(interpolation: Interpolation) -> Self {
        Self {
            keys: Vec::new(),
            interpolation,
        }
    }

    pub fn constant(value: Quat) -> Self {
        Self::new(Interpolation::Step).with_keys(vec![Keyframe::new(0.0, value)])
    }

    pub fn with_keys(mut self, keys: Vec<Keyframe<Quat>>) -> Self {
        self.keys = keys;
        self.keys.sort_by(|a, b| a.time.total_cmp(&b.time));
        self
    }

    pub fn add(&mut self, key: Keyframe<Quat>) {
        let pos = self
            .keys
            .binary_search_by(|k| k.time.total_cmp(&key.time))
            .unwrap_or_else(|i| i);
        self.keys.insert(pos, key);
    }

    pub fn duration(&self) -> f32 {
        self.keys.last().map(|k| k.time).unwrap_or(0.0)
    }

    pub fn sample(&self, time: f32) -> Quat {
        let times: Vec<f32> = self.keys.iter().map(|k| k.time).collect();
        let i = match find_segment(&times, time) {
            Some(i) => i,
            None => return Quat::IDENTITY,
        };
        let t = match normalize_parameter(time, &times, i) {
            Some(t) => t,
            None => return self.keys[self.keys.len() - 1].value,
        };

        let a = &self.keys[i];
        let b = &self.keys[i + 1];
        match self.interpolation {
            Interpolation::Step => a.value,
            Interpolation::Linear | Interpolation::Cubic => a.value.slerp(&b.value, t),
        }
    }
}

/// Translation + rotation + scale for one bone, sampled together.
#[derive(Debug, Clone)]
pub struct TransformTrack {
    pub translation: VectorTrack,
    pub rotation: QuaternionTrack,
    pub scale: VectorTrack,
}

impl TransformTrack {
    pub fn new(interpolation: Interpolation) -> Self {
        Self {
            translation: VectorTrack::new(interpolation),
            rotation: QuaternionTrack::new(interpolation),
            scale: VectorTrack::constant(Vec3::ONE),
        }
    }

    pub fn with_rotation_keys(mut self, keys: Vec<Keyframe<Quat>>) -> Self {
        self.rotation = QuaternionTrack::new(Interpolation::Linear).with_keys(keys);
        self
    }

    pub fn with_translation_keys(mut self, keys: Vec<Keyframe<Vec3>>) -> Self {
        self.translation = VectorTrack::new(Interpolation::Linear).with_keys(keys);
        self
    }

    pub fn duration(&self) -> f32 {
        self.translation
            .duration()
            .max(self.rotation.duration())
            .max(self.scale.duration())
    }

    pub fn sample(&self, time: f32) -> Transform {
        Transform::new(
            self.translation.sample(time),
            self.rotation.sample(time),
            self.scale.sample(time),
        )
    }
}
