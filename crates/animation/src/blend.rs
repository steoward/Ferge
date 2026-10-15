//! Pose representation and the blending operations an engine runs per frame.
//!
//! A `Pose` holds local-space transforms plus the flags a blended pose needs
//! to carry: which bones are actually authored by a clip, so that additive
//! layers only affect the bones the layer touched.

use crate::skeleton::Skeleton;
use crate::clip::ClipSample;
use ow_math::{Quat, Transform, Vec3};

/// A local-space pose: one transform per bone.
#[derive(Debug, Clone)]
pub struct Pose {
    pub locals: Vec<Transform>,
}

impl Pose {
    pub fn identity(bone_count: usize) -> Self {
        Self {
            locals: vec![Transform::identity(); bone_count],
        }
    }

    pub fn from_clip(sample: &ClipSample) -> Self {
        Self {
            locals: sample.locals.clone(),
        }
    }

    pub fn bone_count(&self) -> usize {
        self.locals.len()
    }

    /// Linear blend of two poses, bone by bone. Translation and scale are
    /// lerped; rotation is slerped.
    pub fn blend(&self, other: &Self, t: f32) -> Self {
        let locals = self
            .locals
            .iter()
            .zip(other.locals.iter())
            .map(|(a, b)| {
                Transform::new(
                    a.position.lerp(&b.position, t),
                    a.rotation.slerp(&b.rotation, t),
                    a.scale.lerp(&b.scale, t),
                )
            })
            .collect();
        Self { locals }
    }
}

/// Free-function form of `Pose::blend`, for pipelines that keep poses in
/// separate buffers.
pub fn pose_blend(a: &Pose, b: &Pose, t: f32) -> Pose {
    a.blend(b, t)
}

/// Additive blending: `delta` is the difference between a (usually bind) pose
/// and a layer pose, scaled by `weight` and added onto `base`.
///
/// Only bones authored in `delta` receive the change; the rest of `base`
/// passes through untouched. This is how an engine layers a "look around"
/// additive animation over a locomotion clip without disturbing the legs.
pub fn pose_additive(base: &Pose, delta: &Pose, weight: f32) -> Pose {
    let locals = base
        .locals
        .iter()
        .zip(delta.locals.iter())
        .map(|(b, d)| {
            // Rotational delta: q_delta = q_layer^-1 * q_base, scaled by weight.
            let rot_delta = (d.rotation * b.rotation.conjugate()).normalize();
            // For weight < 1, scale the rotation by interpolating toward identity.
            let scaled_rot = if rot_delta.dot(&Quat::IDENTITY) < 0.0 {
                // Negative scale: negate to take the short arc.
                let flipped = rot_delta * -1.0;
                Quat::IDENTITY.slerp(&flipped, weight)
            } else {
                Quat::IDENTITY.slerp(&rot_delta, weight)
            };

            Transform::new(
                Vec3::new(
                    b.position.x + d.position.x * weight,
                    b.position.y + d.position.y * weight,
                    b.position.z + d.position.z * weight,
                ),
                (scaled_rot * b.rotation).normalize(),
                Vec3::new(
                    b.scale.x * d.scale.x.max(0.0001),
                    b.scale.y * d.scale.y.max(0.0001),
                    b.scale.z * d.scale.z.max(0.0001),
                ),
            )
        })
        .collect();

    Pose { locals }
}

/// Cross-fades between two poses using a smoothstep falloff, which avoids the
/// linear-fade artefact where two clips briefly cancel each other's motion.
pub fn cross_fade(a: &Pose, b: &Pose, t: f32) -> Pose {
    let t = t.clamp(0.0, 1.0);
    let smooth = t * t * (3.0 - 2.0 * t);
    a.blend(b, smooth)
}

/// Weight mask: per-bone blend weights, used to restrict a blend to a subtree.
#[derive(Debug, Clone)]
pub struct BlendMask {
    weights: Vec<f32>,
}

impl BlendMask {
    /// A mask that blends everything at full weight.
    pub fn all(bone_count: usize) -> Self {
        Self {
            weights: vec![1.0; bone_count],
        }
    }

    /// A mask filled with `value`.
    pub fn filled(bone_count: usize, value: f32) -> Self {
        Self {
            weights: vec![value.clamp(0.0, 1.0); bone_count],
        }
    }

    /// Builds a mask from a skeleton where only `bone` and its descendants are
    /// affected, with smooth falloff over `falloff` levels.
    pub fn from_subtree(skeleton: &Skeleton, bone: usize, falloff: usize) -> Self {
        let mut weights = vec![0.0; skeleton.len()];

        let depths = skeleton.depths();
        let base_depth = depths[bone];

        for (i, w) in weights.iter_mut().enumerate() {
            if !is_descendant_of(skeleton, i, bone) {
                continue;
            }
            // The bone itself is unaffected; each level down fades linearly
            // until `falloff` levels below, where the weight reaches zero.
            let extra = depths[i] - base_depth;
            *w = if falloff == 0 {
                1.0
            } else {
                1.0 - (extra as f32 / falloff as f32).min(1.0)
            };
        }

        Self { weights }
    }

    pub fn weight(&self, bone: usize) -> f32 {
        self.weights.get(bone).copied().unwrap_or(1.0)
    }

    pub fn set_weight(&mut self, bone: usize, value: f32) {
        if bone < self.weights.len() {
            self.weights[bone] = value.clamp(0.0, 1.0);
        }
    }

    /// Blends the pose toward `reference` by `1 - weight` for each bone.
    ///
    /// This is the standard blend-mask semantics: a weight of 0 leaves the bone
    /// exactly at `reference` (commonly the bind pose or the clip being masked
    /// out), and a weight of 1 leaves it untouched. Scaling the absolute
    /// transform instead would collapse masked-out bones to the origin.
    pub fn apply(&self, pose: &mut Pose, reference: &Pose) {
        let n = pose.locals.len().min(reference.locals.len());
        for i in 0..n {
            let w = self.weights[i];
            if w >= 1.0 {
                continue;
            }
            pose.locals[i] = reference.locals[i].lerp(&pose.locals[i], w);
        }
    }
}

/// Whether `node` is `ancestor` or lies beneath it.
fn is_descendant_of(skeleton: &Skeleton, node: usize, ancestor: usize) -> bool {
    let mut current = node;
    loop {
        if current == ancestor {
            return true;
        }
        let parent = skeleton.bones[current].parent;
        if parent == u16::MAX {
            return false;
        }
        current = parent as usize;
    }
}
