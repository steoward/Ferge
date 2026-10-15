//! Bone hierarchy.
//!
//! A skeleton is a flat array of bones with parent indices, which keeps the
//! data cache-friendly and lets GPU-side skinning consume it directly.

use ow_math::{Quat, Vec3, Transform};

/// Bones are referenced by index; a parent of `u16::MAX` means "root".
pub const NO_PARENT: u16 = u16::MAX;

/// Per-bone metadata.
#[derive(Debug, Clone)]
pub struct Bone {
    pub name: String,
    pub parent: u16,
    /// Bind-pose transform in model space (also known as the inverse bind pose
    /// inverted: this is the bone's rest transform).
    pub bind_local: Transform,
    /// Bind-pose transform in model space.
    pub bind_model: Transform,
    /// Inverse of `bind_model`, used to carry vertices from model space into
    /// bind-pose bone space for skinning.
    pub inverse_bind: Transform,
    pub flags: BoneFlags,
}

impl Bone {
    pub fn new(name: impl Into<String>, parent: u16) -> Self {
        Self {
            name: name.into(),
            parent,
            bind_local: Transform::identity(),
            bind_model: Transform::identity(),
            inverse_bind: Transform::identity(),
            flags: BoneFlags::empty(),
        }
    }

    pub fn with_bind_local(mut self, t: Transform) -> Self {
        self.bind_local = t;
        self
    }

    pub fn with_bind_model(mut self, t: Transform) -> Self {
        self.bind_model = t;
        self.inverse_bind = t.inverse();
        self
    }

    pub fn is_root(&self) -> bool {
        self.parent == NO_PARENT
    }
}

/// A set of boolean bone properties. `bitflags` would be the usual choice;
/// a hand-rolled bitmask keeps the dependency list at zero while offering the
/// same ergonomics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct BoneFlags(pub u8);

impl BoneFlags {
    pub const NONE: Self = Self(0);
    /// This bone and its subtree are ignored by skinning.
    pub const SKIP_MESH: Self = Self(1 << 0);
    /// This bone is a procedural/IK target rather than a real joint.
    pub const IKTARGET: Self = Self(1 << 1);
    /// This bone should not be rotated by procedural animation.
    pub const LOCK_ROTATION: Self = Self(1 << 2);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    pub fn set(&mut self, other: Self, value: bool) {
        if value {
            self.insert(other);
        } else {
            self.remove(other);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }
}

/// A complete skeleton: an ordered list of bones where every parent precedes
/// its children, so a single forward pass can compute model-space transforms.
#[derive(Debug, Clone)]
pub struct Skeleton {
    pub bones: Vec<Bone>,
}

impl Skeleton {
    pub fn new() -> Self {
        Self { bones: Vec::new() }
    }

    /// Appends a root bone.
    pub fn add_root(&mut self, name: impl Into<String>) -> usize {
        self.add_bone(name, NO_PARENT)
    }

    /// Appends a child bone of `parent`.
    pub fn add_child(&mut self, name: impl Into<String>, parent: usize) -> usize {
        assert!(parent < self.bones.len(), "parent index out of range");
        self.add_bone(name, parent as u16)
    }

    fn add_bone(&mut self, name: impl Into<String>, parent: u16) -> usize {
        let index = self.bones.len();
        self.bones.push(Bone::new(name, parent));
        index
    }

    pub fn bone_index(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|b| b.name == name)
    }

    pub fn bone(&self, index: usize) -> Option<&Bone> {
        self.bones.get(index)
    }

    pub fn len(&self) -> usize {
        self.bones.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bones.is_empty()
    }

    /// Recomputes every bone's `bind_model` and `inverse_bind` from the
    /// per-bone `bind_local` transforms, walking parents before children.
    pub fn recompute_bind_pose(&mut self) {
        for i in 0..self.bones.len() {
            let parent = self.bones[i].parent;
            let local = self.bones[i].bind_local;
            let model = if parent == NO_PARENT {
                local
            } else {
                self.bones[parent as usize].bind_model * local
            };
            self.bones[i].bind_model = model;
            self.bones[i].inverse_bind = model.inverse();
        }
    }

    /// The rest-pose model-space transforms, which are the identity of the
    /// skinning problem.
    pub fn bind_pose_matrices(&self) -> Vec<ow_math::Mat4> {
        self.bones
            .iter()
            .map(|b| (b.bind_model * b.inverse_bind).to_mat4())
            .collect()
    }

    /// Depth of each bone below its root. Roots have depth 0.
    pub fn depths(&self) -> Vec<usize> {
        self.bones
            .iter()
            .map(|bone| {
                let mut depth = 0;
                let mut current = bone.parent;
                while current != NO_PARENT {
                    depth += 1;
                    current = self.bones[current as usize].parent;
                }
                depth
            })
            .collect()
    }

    /// Whether the hierarchy is a forest (no cycles, every parent precedes its
    /// child). Malformed skeletons silently corrupt model-space transforms, so
    /// this is worth validating after loading.
    pub fn is_valid(&self) -> bool {
        for (i, bone) in self.bones.iter().enumerate() {
            if bone.parent == NO_PARENT {
                continue;
            }
            if bone.parent as usize >= i {
                return false;
            }
        }
        true
    }

    /// Direct child indices of bone `bone`.
    pub fn children(&self, bone: usize) -> Vec<usize> {
        self.bones
            .iter()
            .enumerate()
            .filter(|(i, b)| *i != bone && b.parent as usize == bone)
            .map(|(i, _)| i)
            .collect()
    }
}

impl Default for Skeleton {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience: forward kinematics — turn per-bone local transforms into
/// model-space transforms.
pub fn local_to_model(skeleton: &Skeleton, locals: &[Transform]) -> Vec<Transform> {
    let mut model: Vec<Transform> = Vec::with_capacity(locals.len());
    for (i, local) in locals.iter().enumerate() {
        let parent = skeleton.bones[i].parent;
        let m = if parent == NO_PARENT {
            *local
        } else {
            model[parent as usize] * *local
        };
        model.push(m);
    }
    model
}

/// Quaternion-only forward kinematics, for when scale is known to be uniform.
pub fn local_to_model_quat(
    skeleton: &Skeleton,
    positions: &[Vec3],
    rotations: &[Quat],
) -> (Vec<Vec3>, Vec<Quat>) {
    let mut out_pos: Vec<Vec3> = Vec::with_capacity(positions.len());
    let mut out_rot: Vec<Quat> = Vec::with_capacity(rotations.len());

    for (i, position) in positions.iter().enumerate() {
        let parent = skeleton.bones[i].parent;
        let (p, r) = if parent == NO_PARENT {
            (*position, rotations[i])
        } else {
            let parent_index = parent as usize;
            let pp: Vec3 = out_pos[parent_index];
            let pr: Quat = out_rot[parent_index];
            (
                pp + pr.rotate_vector(*position),
                pr * rotations[i],
            )
        };
        out_pos.push(p);
        out_rot.push(r);
    }
    (out_pos, out_rot)
}
