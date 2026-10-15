//! Animation blend trees.
//!
//! A blend tree is the data-driven way engines combine clips: locomotion
//! blends direction and speed into a 1D or 2D grid of clips, and the tree is
//! evaluated each frame into a pose. Keeping it as data (rather than code)
//! lets designers tune blends without a rebuild.

use crate::blend::{cross_fade, pose_additive, pose_blend, Pose};

/// A node in a blend tree.
#[derive(Debug, Clone)]
pub enum BlendNode {
    /// Samples the clip with this index at raw playback `time`.
    Clip { clip: usize },
    /// Linear blend of two or more children by an axis parameter.
    Blend1D {
        children: Vec<BlendNode>,
        /// Parameter values matching each child, ascending.
        positions: Vec<f32>,
        t: f32,
    },
    /// Bilinear blend of four children by two parameters.
    ///
    /// Children are row-major: `[0]=(x0,y0)`, `[1]=(x1,y0)`,
    /// `[2]=(x0,y1)`, `[3]=(x1,y1)`.
    Blend2D {
        children: Vec<BlendNode>,
        x_range: (f32, f32),
        y_range: (f32, f32),
        x: f32,
        y: f32,
    },
    /// Additive layer over a base.
    Additive {
        base: Box<BlendNode>,
        layer: Box<BlendNode>,
        weight: f32,
    },
    /// Cross-fade between two children using smoothstep.
    CrossFade {
        a: Box<BlendNode>,
        b: Box<BlendNode>,
        t: f32,
    },
}

/// Evaluates a blend tree into a pose.
///
/// `sample_fn(clip, time) -> Pose` is supplied by the host so the tree stays
/// independent of clip storage; typically it delegates to an
/// `AnimationClip::sample`.
pub fn evaluate(
    node: &BlendNode,
    bone_count: usize,
    time: f32,
    sample_fn: &mut impl FnMut(usize, f32) -> Pose,
) -> Pose {
    match node {
        BlendNode::Clip { clip } => sample_fn(*clip, time),

        BlendNode::Blend1D { children, positions, t } => {
            blend_1d(children, positions, *t, bone_count, time, sample_fn)
        }

        BlendNode::Blend2D {
            children,
            x_range,
            y_range,
            x,
            y,
        } => {
            // A 2D grid needs exactly four corners.
            if children.len() != 4 {
                return Pose::identity(bone_count);
            }
            let mut boxed: [Box<BlendNode>; 4] = [
                Box::new(BlendNode::Clip { clip: 0 }),
                Box::new(BlendNode::Clip { clip: 0 }),
                Box::new(BlendNode::Clip { clip: 0 }),
                Box::new(BlendNode::Clip { clip: 0 }),
            ];
            for (slot, child) in boxed.iter_mut().zip(children.iter()) {
                **slot = child.clone();
            }
            blend_2d(
                &Blend2DInput {
                    children: boxed,
                    x_range: *x_range,
                    y_range: *y_range,
                    x: *x,
                    y: *y,
                },
                bone_count,
                time,
                sample_fn,
            )
        }

        BlendNode::Additive { base, layer, weight } => {
            let base_pose = evaluate(base, bone_count, time, sample_fn);
            let layer_pose = evaluate(layer, bone_count, time, sample_fn);
            pose_additive(&base_pose, &layer_pose, *weight)
        }

        BlendNode::CrossFade { a, b, t } => {
            let pa = evaluate(a, bone_count, time, sample_fn);
            let pb = evaluate(b, bone_count, time, sample_fn);
            cross_fade(&pa, &pb, *t)
        }
    }
}

fn blend_1d(
    children: &[BlendNode],
    positions: &[f32],
    t: f32,
    bone_count: usize,
    time: f32,
    sample_fn: &mut impl FnMut(usize, f32) -> Pose,
) -> Pose {
    if children.is_empty() {
        return Pose::identity(bone_count);
    }
    if positions.len() != children.len() {
        return evaluate(&children[0], bone_count, time, sample_fn);
    }
    if children.len() == 1 {
        return evaluate(&children[0], bone_count, time, sample_fn);
    }

    // Find the segment containing `t`, clamping at both ends.
    let mut i = 0;
    while i + 1 < positions.len() && t > positions[i + 1] {
        i += 1;
    }
    if i + 1 >= children.len() {
        return evaluate(&children[children.len() - 1], bone_count, time, sample_fn);
    }

    let span = positions[i + 1] - positions[i];
    let alpha = if span.abs() > 1e-6 {
        ((t - positions[i]) / span).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let pa = evaluate(&children[i], bone_count, time, sample_fn);
    let pb = evaluate(&children[i + 1], bone_count, time, sample_fn);
    pose_blend(&pa, &pb, alpha)
}

/// Everything a 2D blend needs: the four child poses plus the two parameter
/// axes. Bundled so the blend step stays under the argument-count lint.
struct Blend2DInput {
    children: [Box<BlendNode>; 4],
    x_range: (f32, f32),
    y_range: (f32, f32),
    x: f32,
    y: f32,
}

fn blend_2d(
    input: &Blend2DInput,
    bone_count: usize,
    time: f32,
    sample_fn: &mut impl FnMut(usize, f32) -> Pose,
) -> Pose {
    let tx = parameter(input.x, input.x_range);
    let ty = parameter(input.y, input.y_range);

    let bottom = {
        let p0 = evaluate(&input.children[0], bone_count, time, sample_fn);
        let p1 = evaluate(&input.children[1], bone_count, time, sample_fn);
        pose_blend(&p0, &p1, tx)
    };
    let top = {
        let p2 = evaluate(&input.children[2], bone_count, time, sample_fn);
        let p3 = evaluate(&input.children[3], bone_count, time, sample_fn);
        pose_blend(&p2, &p3, tx)
    };

    pose_blend(&bottom, &top, ty)
}

/// Normalises `value` into `[0, 1]` across `range`, clamped at both ends.
fn parameter(value: f32, range: (f32, f32)) -> f32 {
    let span = range.1 - range.0;
    if span.abs() > 1e-6 {
        ((value - range.0) / span).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// A 1D blend axis, for authoring readable tree definitions.
#[derive(Debug, Clone)]
pub struct Blend1D {
    pub clips: Vec<usize>,
    /// Parameter value per clip, ascending.
    pub positions: Vec<f32>,
}

impl Blend1D {
    pub fn new(clips: Vec<usize>, positions: Vec<f32>) -> Self {
        Self { clips, positions }
    }

    /// Builds the corresponding tree node.
    pub fn to_node(&self) -> BlendNode {
        BlendNode::Blend1D {
            children: self.clips.iter().map(|c| BlendNode::Clip { clip: *c }).collect(),
            positions: self.positions.clone(),
            t: 0.0,
        }
    }
}

/// A 2D blend grid, for direction × speed locomotions.
#[derive(Debug, Clone)]
pub struct Blend2D {
    /// Row-major 2×2 grid of clip indices.
    pub clips: [usize; 4],
    pub x_range: (f32, f32),
    pub y_range: (f32, f32),
}

impl Blend2D {
    pub fn new(clips: [usize; 4], x_range: (f32, f32), y_range: (f32, f32)) -> Self {
        Self { clips, x_range, y_range }
    }

    pub fn to_node(&self) -> BlendNode {
        BlendNode::Blend2D {
            children: self.clips.iter().map(|c| BlendNode::Clip { clip: *c }).collect(),
            x_range: self.x_range,
            y_range: self.y_range,
            x: 0.0,
            y: 0.0,
        }
    }
}

/// A complete animation graph.
#[derive(Debug, Clone)]
pub struct BlendTree {
    pub root: BlendNode,
}

impl BlendTree {
    pub fn new(root: BlendNode) -> Self {
        Self { root }
    }

    pub fn evaluate(
        &self,
        bone_count: usize,
        time: f32,
        sample_fn: &mut impl FnMut(usize, f32) -> Pose,
    ) -> Pose {
        evaluate(&self.root, bone_count, time, sample_fn)
    }
}
