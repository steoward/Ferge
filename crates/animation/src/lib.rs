//! Skeletal animation: bone hierarchies, keyframe tracks, pose blending,
//! additive layers, and two-bone inverse kinematics.
//!
//! The pipeline is the standard one used by every skinned-mesh engine:
//!
//! ```text
//! keyframe tracks (pos/rot/scale) --sample--> local bone transforms
//!                                           --local->model--> model transforms
//!                                           --inverse bind--> skinning matrices
//!                                           --apply--> deformed vertices
//! ```

pub mod skeleton;
pub mod track;
pub mod clip;
pub mod blend;
pub mod ik;
pub mod skinning;
pub mod blend_tree;

pub use skeleton::{Skeleton, Bone, BoneFlags};
pub use track::{Keyframe, VectorTrack, QuaternionTrack, TransformTrack, Interpolation};
pub use clip::{AnimationClip, ClipSample};
pub use blend::{Pose, pose_blend, pose_additive, cross_fade};
pub use ik::{solve_two_bone_ik, aim_constraint};
pub use skinning::{compute_skinning_matrices, apply_skinning, Skin};
pub use blend_tree::{BlendNode, BlendTree, Blend1D, Blend2D, evaluate};
