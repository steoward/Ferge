//! Inverse kinematics.
//!
//! Two-bone IK (a.k.a. analytic CCD for a single chain) is closed form: given
//! the two bone lengths and the target distance, the interior angle follows
//! from the law of cosines. This is exact and O(1), unlike iterative CCD, and
//! is what engines use for legs, arms and look-at targets.
//!
//! The aim constraint is the simpler problem of orienting a single bone so its
//! forward axis points at a target.

use ow_math::{Quat, Vec3};

/// Environment a two-bone chain is solved in.
#[derive(Debug, Clone, Copy)]
pub struct TwoBoneChain {
    /// Root joint position in model space.
    pub root: Vec3,
    /// Length of the upper bone (root -> middle).
    pub upper_length: f32,
    /// Length of the lower bone (middle -> end).
    pub lower_length: f32,
    /// Axis the chain's rest pose points along, in model space.
    pub rest_axis: Vec3,
    /// Pole vector the bend plane should align with. Defaults to `+Y`.
    pub pole: Vec3,
}

/// The solved configuration of a two-bone chain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwoBoneSolution {
    /// New middle joint position.
    pub middle: Vec3,
    /// New end-effector position (clamped to the reachable annulus).
    pub end: Vec3,
    /// Whether the target was reachable; `false` means the chain was fully
    /// extended and `end` is the closest reachable point.
    pub reachable: bool,
}

/// Normalizes a vector, falling back to a perpendicular basis vector when the
/// input is degenerate so callers never have to handle a zero direction.
fn safe_normalize(v: Vec3, fallback: Vec3) -> Vec3 {
    let n = v.normalize();
    if n.length_squared() < 1e-8 {
        fallback
    } else {
        n
    }
}

/// Solves a two-bone chain so its end-effector reaches `target`.
///
/// The interior angle at the middle joint comes from the law of cosines, so
/// the result is exact rather than iterated to convergence.
pub fn solve_two_bone_ik(chain: &TwoBoneChain, target: Vec3) -> TwoBoneSolution {
    let a = chain.upper_length;
    let b = chain.lower_length;

    // Clamp the target to the reachable annulus [|a-b|, a+b].
    let to_target = target - chain.root;
    let dist = to_target.length();
    let max_reach = a + b;
    let min_reach = (a - b).abs();

    let reachable = dist <= max_reach + 1e-6 && dist >= min_reach - 1e-6;

    let dir = safe_normalize(to_target, chain.rest_axis);

    // Distance along root->target where the middle joint sits.
    let clamped_dist = dist.clamp(min_reach.max(1e-6), max_reach.max(1e-6));
    let clamped_target = chain.root + dir * clamped_dist;

    // Law of cosines for the angle at the root between the upper bone and the
    // root->target direction.
    let cos_root = if clamped_dist > 1e-6 {
        ((a * a + clamped_dist * clamped_dist - b * b) / (2.0 * a * clamped_dist)).clamp(-1.0, 1.0)
    } else {
        1.0
    };
    let root_angle = cos_root.acos();

    // Build an orthonormal basis for the bend plane: forward = root->target,
    // side = the pole direction made perpendicular to it.
    let forward = safe_normalize(clamped_target - chain.root, chain.rest_axis);
    let pole = safe_normalize(
        chain.pole - forward * chain.pole.dot(&forward),
        Vec3::RIGHT,
    );
    let side = safe_normalize(pole.cross(forward), Vec3::RIGHT);

    // Middle joint: rotate `forward` by root_angle around `side`, then scale.
    let rotation = Quat::from_axis_angle(side, root_angle);
    let upper_dir = rotation.rotate_vector(forward).normalize();
    let middle = chain.root + upper_dir * a;

    TwoBoneSolution {
        middle,
        end: clamped_target,
        reachable,
    }
}

/// Orients a single bone so its local `+Z` axis points at `target`.
///
/// Returns identity when the target coincides with the bone position.
pub fn aim_constraint(from: Vec3, to: Vec3, fallback_up: Vec3) -> Quat {
    let dir = safe_normalize(to - from, Vec3::FORWARD);
    let forward = safe_normalize(Vec3::FORWARD, Vec3::FORWARD);

    if (dir - forward).length_squared() < 1e-8 {
        return Quat::IDENTITY;
    }
    if (dir + forward).length_squared() < 1e-8 {
        // Target is directly behind: any 180° turn works, pick one around `up`.
        let axis = safe_normalize(fallback_up, Vec3::UP);
        return Quat::from_axis_angle(axis, std::f32::consts::PI);
    }

    // Shortest-arc rotation from `forward` to `dir`.
    let axis = forward.cross(dir);
    let angle = forward.dot(&dir).acos().clamp(0.0, std::f32::consts::PI);
    Quat::from_axis_angle(axis.normalize(), angle)
}

/// Iterative cyclic-coordinate-descent IK for chains longer than two bones.
/// Two-bone chains should use [`solve_two_bone_ik`] instead, which is exact.
///
/// Each iteration rotates the joint closest to the end-effector so that its
/// segment points at the target, walking back toward the root. Converges
/// quickly for well-conditioned chains.
pub fn solve_ccd_ik(
    positions: &mut [Vec3],
    lengths: &[f32],
    target: Vec3,
    iterations: usize,
    tolerance: f32,
) -> f32 {
    if positions.len() != lengths.len() + 1 {
        return (positions[positions.len() - 1] - target).length();
    }

    let mut best_error = f32::INFINITY;

    for _ in 0..iterations {
        let mut error = (positions[positions.len() - 1] - target).length();
        if error < tolerance {
            break;
        }

        // Walk from the joint nearest the end-effector back to the root.
        for i in (0..lengths.len()).rev() {
            let joint = positions[i];
            let end = positions[i + 1];
            let to_target = target - joint;
            let to_end = end - joint;

            if to_target.length_squared() < 1e-12 || to_end.length_squared() < 1e-12 {
                continue;
            }

            let to_target = to_target.normalize();
            let to_end = to_end.normalize();

            let axis = to_end.cross(to_target);
            let sin = axis.length();
            if sin < 1e-9 {
                continue;
            }
            let angle = to_end.dot(&to_target).acos().clamp(-1.0, 1.0);
            let axis = axis / sin;

            let rotation = Quat::from_axis_angle(axis, angle);
            // Rotate every joint downstream of `i`.
            for downstream in positions[(i + 1)..].iter_mut() {
                let rel = *downstream - joint;
                *downstream = joint + rotation.rotate_vector(rel);
            }

            let new_error = (positions[positions.len() - 1] - target).length();
            if new_error < error {
                error = new_error;
            }
        }

        best_error = error;
        if best_error < tolerance {
            break;
        }
    }

    best_error
}
