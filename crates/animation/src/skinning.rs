//! Linear blend skinning.
//!
//! For each vertex:
//!
//! ```text
//! v' = Σ_i w_i * (M_i * v)
//! ```
//!
//! where `M_i = bind_model_i * inverse_bind_i` is the bone's skinning matrix.
//! Each vertex's weights must sum to 1 for the result to stay rigid; the
//! normal is transformed by the same matrices (ignoring the non-uniform scale
//! correction, which is only visible with strong scaling).

use ow_math::{Mat4, Quat, Transform, Vec3};

/// Skinning weight for one bone influence on a vertex.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoneWeight {
    pub bone_index: u16,
    pub weight: f32,
}

/// Per-vertex skinning data: up to `MAX_INFLUENCES` bone weights.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VertexInfluences {
    pub weights: [BoneWeight; MAX_INFLUENCES],
}

/// Number of bone influences per vertex. Four is conventional: it fits the
/// common 8-float vertex attribute layout (2× RGBA16) and covers almost every
/// real mesh, and any extra influences can be baked into the closest ones.
pub const MAX_INFLUENCES: usize = 4;

impl Default for VertexInfluences {
    fn default() -> Self {
        Self {
            weights: [BoneWeight {
                bone_index: 0,
                weight: 0.0,
            }; MAX_INFLUENCES],
        }
    }
}

impl VertexInfluences {
    /// Adds an influence, replacing the weakest existing one when full.
    pub fn push(&mut self, bone_index: u16, weight: f32) {
        let w = weight.max(0.0);

        // If already present, just accumulate.
        for slot in &mut self.weights {
            if slot.bone_index == bone_index {
                slot.weight += w;
                return;
            }
        }

        // Otherwise find the weakest slot and overwrite it if the new weight
        // is stronger — this keeps the N most significant influences.
        let mut weakest = 0usize;
        for i in 1..MAX_INFLUENCES {
            if self.weights[i].weight < self.weights[weakest].weight {
                weakest = i;
            }
        }
        if w > self.weights[weakest].weight {
            self.weights[weakest] = BoneWeight { bone_index, weight: w };
        }
    }

    /// Renormalises the weights so they sum to exactly 1. Meshes that come in
    /// with non-normalised weights (common after quantisation) would otherwise
    /// drift vertices away from the bind pose.
    pub fn normalize(&mut self) {
        let sum: f32 = self.weights.iter().map(|w| w.weight).sum();
        if sum <= 1e-8 {
            return;
        }
        let inv = 1.0 / sum;
        for w in &mut self.weights {
            w.weight *= inv;
        }
    }

    pub fn influence_count(&self) -> usize {
        self.weights.iter().filter(|w| w.weight > 0.0).count()
    }

    pub fn total_weight(&self) -> f32 {
        self.weights.iter().map(|w| w.weight).sum()
    }
}

/// A skinned mesh: vertices plus the skeleton driving them.
#[derive(Debug, Clone)]
pub struct Skin {
    /// One entry per vertex.
    pub influences: Vec<VertexInfluences>,
}

impl Skin {
    pub fn new(influences: Vec<VertexInfluences>) -> Self {
        Self { influences }
    }

    pub fn vertex_count(&self) -> usize {
        self.influences.len()
    }

    /// Renormalises every vertex's weights.
    pub fn normalize_all(&mut self) {
        for v in &mut self.influences {
            v.normalize();
        }
    }
}

/// Computes `bind_model * inverse_bind` for every bone — the matrices that
/// carry a vertex from bind-pose bone space into the current animated pose.
///
/// Both arguments are in the skeleton's native `Transform` form, matching the
/// bone hierarchy's own data, and are converted to matrices here.
pub fn compute_skinning_matrices(
    bind_model: &[Transform],
    inverse_bind: &[Transform],
) -> Vec<Mat4> {
    bind_model
        .iter()
        .zip(inverse_bind.iter())
        .map(|(model, inv_bind)| model.to_mat4() * inv_bind.to_mat4())
        .collect()
}

/// Applies skinning to positions, in place.
pub fn apply_skinning_positions(
    skin: &Skin,
    base_positions: &[Vec3],
    matrices: &[Mat4],
    out_positions: &mut [Vec3],
) {
    for (i, influence) in skin.influences.iter().enumerate() {
        let base = base_positions[i];

        if influence.influence_count() == 0 {
            out_positions[i] = base;
            continue;
        }

        let mut accumulated = Vec3::ZERO;
        for w in &influence.weights {
            if w.weight == 0.0 {
                continue;
            }
            let m = &matrices[w.bone_index as usize];
            accumulated = accumulated + m.transform_point(base) * w.weight;
        }
        out_positions[i] = accumulated;
    }
}

/// Applies skinning to normals, in place.
///
/// The normal matrix should be the inverse transpose of the skinning matrix to
/// survive non-uniform scale. Recomputing an inverse per vertex is far too
/// costly, so this uses the same 3×3 rotation block — which is exact whenever
/// the bone's scale is uniform (the overwhelmingly common case).
pub fn apply_skinning_normals(
    skin: &Skin,
    base_normals: &[Vec3],
    matrices: &[Mat4],
    out_normals: &mut [Vec3],
) {
    for (i, influence) in skin.influences.iter().enumerate() {
        let base = base_normals[i];

        if influence.influence_count() == 0 {
            out_normals[i] = base;
            continue;
        }

        let mut accumulated = Vec3::ZERO;
        for w in &influence.weights {
            if w.weight == 0.0 {
                continue;
            }
            let m = &matrices[w.bone_index as usize];
            accumulated = accumulated + m.transform_vector(base) * w.weight;
        }
        out_normals[i] = accumulated.normalize();
    }
}

/// Convenience wrapper: skin both positions and normals.
pub fn apply_skinning(
    skin: &Skin,
    base_positions: &[Vec3],
    base_normals: &[Vec3],
    matrices: &[Mat4],
) -> (Vec<Vec3>, Vec<Vec3>) {
    let mut pos = vec![Vec3::ZERO; base_positions.len()];
    let mut nor = vec![Vec3::ZERO; base_normals.len()];
    apply_skinning_positions(skin, base_positions, matrices, &mut pos);
    apply_skinning_normals(skin, base_normals, matrices, &mut nor);
    (pos, nor)
}

/// Total warp magnitude: the largest distance a vertex moved from its bind
/// pose position. Useful to detect badly skinned meshes and to decide when a
/// skinned mesh must be re-uploaded to the GPU.
pub fn total_warp(base: &[Vec3], deformed: &[Vec3]) -> f32 {
    base.iter()
        .zip(deformed.iter())
        .map(|(b, d)| (*d - *b).length())
        .fold(0.0f32, f32::max)
}

/// A bone's rotation and translation extracted from a matrix, used when
/// importing or re-exporting poses.
pub fn decompose_matrix(m: &Mat4) -> (Vec3, Quat, Vec3) {
    let translation = Vec3::new(m.m[12], m.m[13], m.m[14]);

    // Column lengths give the scale; averaging lateral and vertical is a
    // deliberate approximation that assumes near-uniform scale.
    let sx = Vec3::new(m.m[0], m.m[1], m.m[2]).length();
    let sy = Vec3::new(m.m[4], m.m[5], m.m[6]).length();
    let sz = Vec3::new(m.m[8], m.m[9], m.m[10]).length();
    let scale = Vec3::new(sx, sy, sz);

    // Normalised 3×3 block -> quaternion (Shepperd's method).
    let (sx, sy, sz) = if sx > 0.0 { (sx, sy, sz) } else { (1.0, 1.0, 1.0) };
    let r = [
        [m.m[0] / sx, m.m[4] / sx, m.m[8] / sx],
        [m.m[1] / sy, m.m[5] / sy, m.m[9] / sy],
        [m.m[2] / sz, m.m[6] / sz, m.m[10] / sz],
    ];

    let trace = r[0][0] + r[1][1] + r[2][2];
    // Shepperd's method. `Quat::new` takes (x, y, z, w) — putting w first
    // silently produces a plausible-looking but wrong rotation.
    //
    // The numeric factors are the inverses of what the textbook formulas print:
    // in the trace branch `s = 1/(4w)` so the components multiply, while in the
    // off-diagonal branches `s = 4*q_largest` so the divisions stay divisions.
    let rotation = if trace > 0.0 {
        let s = 0.5 / (trace + 1.0).sqrt();
        Quat::new(
            (r[2][1] - r[1][2]) * s,
            (r[0][2] - r[2][0]) * s,
            (r[1][0] - r[0][1]) * s,
            0.25 / s,
        )
        .normalize()
    } else if r[0][0] > r[1][1] && r[0][0] > r[2][2] {
        let s = 2.0 * (1.0 + r[0][0] - r[1][1] - r[2][2]).sqrt();
        Quat::new(
            0.25 * s,
            (r[0][1] + r[1][0]) / s,
            (r[0][2] + r[2][0]) / s,
            (r[1][2] - r[2][1]) / s,
        )
        .normalize()
    } else if r[1][1] > r[2][2] {
        let s = 2.0 * (1.0 + r[1][1] - r[0][0] - r[2][2]).sqrt();
        Quat::new(
            (r[0][1] + r[1][0]) / s,
            0.25 * s,
            (r[1][2] + r[2][1]) / s,
            (r[0][2] - r[2][0]) / s,
        )
        .normalize()
    } else {
        let s = 2.0 * (1.0 + r[2][2] - r[0][0] - r[1][1]).sqrt();
        Quat::new(
            (r[0][2] + r[2][0]) / s,
            (r[1][2] + r[2][1]) / s,
            0.25 * s,
            (r[0][1] - r[1][0]) / s,
        )
        .normalize()
    };

    (translation, rotation, scale)
}
