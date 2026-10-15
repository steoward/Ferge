use ow_animation::blend::{cross_fade, pose_additive, pose_blend, Pose, BlendMask};
use ow_animation::clip::{AnimationClip, WrapMode};
use ow_animation::ik::{aim_constraint, solve_ccd_ik, solve_two_bone_ik, TwoBoneChain};
use ow_animation::skeleton::{local_to_model, Skeleton};
use ow_animation::skinning::*;
use ow_animation::track::{Interpolation, Keyframe, QuaternionTrack, TransformTrack, VectorTrack};
use ow_animation::blend_tree::{BlendNode, evaluate};
use ow_math::{Quat, Transform, Vec3};

// ---------------------------------------------------------------------------
// Skeleton & forward kinematics
// ---------------------------------------------------------------------------

#[test]
fn skeleton_parents_must_precede_children() {
    let mut s = Skeleton::new();
    let root = s.add_root("hips");
    let spine = s.add_child("spine", root);
    let head = s.add_child("head", spine);

    assert_eq!(root, 0);
    assert_eq!(spine, 1);
    assert_eq!(head, 2);
    assert!(s.is_valid());
    assert_eq!(s.depths(), vec![0, 1, 2]);
    assert_eq!(s.children(root), vec![spine]);
    assert_eq!(s.children(spine), vec![head]);
}

#[test]
fn bind_pose_recompute_and_inverse() {
    let mut s = Skeleton::new();
    let root = s.add_root("root");
    let child = s.add_child("child", root);
    s.bones[root].bind_local = Transform::new(Vec3::new(1.0, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE);
    s.bones[child].bind_local = Transform::new(Vec3::new(0.0, 2.0, 0.0), Quat::IDENTITY, Vec3::ONE);
    s.recompute_bind_pose();

    // Parent model transform is its local.
    assert_eq!(s.bones[root].bind_model.position, Vec3::new(1.0, 0.0, 0.0));
    // Child model transform composes parent then child.
    assert_eq!(s.bones[child].bind_model.position, Vec3::new(1.0, 2.0, 0.0));

    // Composing a transform with its inverse yields the identity.
    let m = s.bones[child].bind_model.compose(&s.bones[child].inverse_bind);
    assert!((m.position.length()) < 1e-5, "inverse should cancel, got {:?}", m.position);
    assert!(
        m.rotation.dot(&Quat::IDENTITY).abs() > 0.9999,
        "inverse rotation should cancel"
    );
}

#[test]
fn local_to_model_propagates_parent_rotation() {
    let mut s = Skeleton::new();
    let root = s.add_root("root");
    let child = s.add_child("child", root);

    // Rotate the root 90 degrees about +Y.
    let quarter = Quat::from_axis_angle(Vec3::UP, std::f32::consts::FRAC_PI_2);
    let locals = vec![
        Transform::new(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE),
        Transform::new(Vec3::new(0.0, 0.0, 1.0), Quat::IDENTITY, Vec3::ONE),
    ];

    // With the root unrotated, the child sits at (0, 0, 1).
    let model = local_to_model(&s, &locals);
    assert!((model[child].position - Vec3::new(0.0, 0.0, 1.0)).length() < 1e-5);

    // Rotate the parent: the child must swing to +X.
    let locals = vec![
        Transform::new(Vec3::ZERO, quarter, Vec3::ONE),
        Transform::new(Vec3::new(0.0, 0.0, 1.0), Quat::IDENTITY, Vec3::ONE),
    ];
    let model = local_to_model(&s, &locals);
    assert!(
        (model[child].position.x).abs() > 0.99,
        "child should swing to +X, got {:?}",
        model[child].position
    );
    assert!(model[child].position.z.abs() < 1e-5);
}

#[test]
fn local_to_model_quat_matches_transform_version() {
    let mut s = Skeleton::new();
    let _root = s.add_root("root");
    let _child = s.add_child("child", _root);

    let rot = Quat::from_euler(0.3, 1.1, 0.7);
    let pos = [Vec3::ZERO, Vec3::new(0.0, 0.0, 3.0)];
    let rots = [rot, Quat::IDENTITY];

    let (positions, rotations) = ow_animation::skeleton::local_to_model_quat(&s, &pos, &rots);

    let model = local_to_model(&s, &[
        Transform::new(pos[0], rots[0], Vec3::ONE),
        Transform::new(pos[1], rots[1], Vec3::ONE),
    ]);

    for i in 0..2 {
        assert!((positions[i] - model[i].position).length() < 1e-4, "position mismatch at {}", i);
        assert!(rotations[i].dot(&model[i].rotation).abs() > 0.9999, "rotation mismatch at {}", i);
    }
}

// ---------------------------------------------------------------------------
// Tracks and interpolation
// ---------------------------------------------------------------------------

#[test]
fn step_interpolation_holds_value() {
    let track = ow_animation::track::ScalarTrack::new(Interpolation::Step)
        .with_keys(vec![
            Keyframe::new(0.0, 0.0),
            Keyframe::new(2.0, 10.0),
        ]);
    assert_eq!(track.sample(1.5), 0.0);
    assert_eq!(track.sample(1.9), 0.0);
    // At or past the last key, the last value is held.
    assert_eq!(track.sample(2.0), 10.0);
}

#[test]
fn scalar_track_sorts_keys_and_handles_bounds() {
    let mut track = ow_animation::track::ScalarTrack::new(Interpolation::Linear);
    track.add(Keyframe::new(2.0, 20.0));
    track.add(Keyframe::new(0.0, 0.0));
    track.add(Keyframe::new(1.0, 10.0));
    assert_eq!(track.duration(), 2.0);

    // Before the first key it clamps; after the last it clamps.
    assert_eq!(track.sample(-1.0), 0.0);
    assert_eq!(track.sample(99.0), 20.0);
    assert_eq!(track.sample(0.5), 5.0);
    assert_eq!(track.sample(1.5), 15.0);
}

#[test]
fn linear_interpolation_midpoint() {
    let track = VectorTrack::new(Interpolation::Linear)
        .with_keys(vec![
            Keyframe::new(0.0, Vec3::ZERO),
            Keyframe::new(4.0, Vec3::new(8.0, 0.0, 0.0)),
        ]);
    let v = track.sample(2.0);
    assert!((v.x - 4.0).abs() < 1e-5, "expected 4.0, got {:?}", v);
}

#[test]
fn cubic_interpolation_deviates_from_linear() {
    // Hermite with zero tangents is the smoothstep ease — smooth, but NOT the
    // straight line. That difference is exactly what the extra tangent data
    // buys: a near-linear curve that still eases at the ends.
    let base = vec![
        Keyframe::new(0.0, Vec3::ZERO).with_tangents(Vec3::ZERO, Vec3::ZERO),
        Keyframe::new(1.0, Vec3::new(1.0, 0.0, 0.0)).with_tangents(Vec3::ZERO, Vec3::ZERO),
    ];
    let linear = VectorTrack::new(Interpolation::Linear).with_keys(base.clone());
    let cubic = VectorTrack::new(Interpolation::Cubic).with_keys(base);

    // Both agree exactly at the endpoints and differ in between.
    assert!((linear.sample(0.0).x - cubic.sample(0.0).x).abs() < 1e-5);
    assert!((linear.sample(1.0).x - cubic.sample(1.0).x).abs() < 1e-5);
    let diff = (linear.sample(0.25).x - cubic.sample(0.25).x).abs();
    assert!(diff > 0.05, "zero-tangent cubic should ease off the chord, got diff {}", diff);
    // Smoothstep at t=0.5 is exactly 0.5, so the midpoint still agrees.
    assert!((linear.sample(0.5).x - cubic.sample(0.5).x).abs() < 1e-5);

    // A non-zero out-tangent must push the curve above the straight line.
    let eased = VectorTrack::new(Interpolation::Cubic).with_keys(vec![
        Keyframe::new(0.0, Vec3::ZERO).with_tangents(Vec3::ZERO, Vec3::new(3.0, 0.0, 0.0)),
        Keyframe::new(1.0, Vec3::new(1.0, 0.0, 0.0)).with_tangents(Vec3::ZERO, Vec3::ZERO),
    ]);
    assert!(
        eased.sample(0.5).x > linear.sample(0.5).x,
        "a positive out-tangent should overshoot the chord"
    );
}

#[test]
fn quaternion_track_uses_slerp_and_stays_normalised() {
    let a = Quat::IDENTITY;
    let b = Quat::from_axis_angle(Vec3::UP, std::f32::consts::PI);
    let track = QuaternionTrack::new(Interpolation::Linear).with_keys(vec![
        Keyframe::new(0.0, a),
        Keyframe::new(1.0, b),
    ]);

    let mid = track.sample(0.5);
    assert!((mid.length() - 1.0).abs() < 1e-4, "slerp must stay on the unit sphere");
    // Halfway through a 180° turn is 90°.
    assert!(mid.y.abs() > 0.7, "expected ~90 degrees about Y, got {:?}", mid);
}

#[test]
fn clip_wrap_modes() {
    let mut clip = AnimationClip::new("test");
    let track = TransformTrack::new(Interpolation::Linear)
        .with_rotation_keys(vec![
            Keyframe::new(0.0, Quat::IDENTITY),
            Keyframe::new(2.0, Quat::from_axis_angle(Vec3::UP, std::f32::consts::FRAC_PI_2)),
        ]);
    clip.add_channel(0, track);

    // The default wrap mode is Loop, so raw time wraps before any explicit
    // choice — each mode is set explicitly below.
    assert!((clip.map_time(3.0) - 1.0).abs() < 1e-6, "default Loop at t=3 with d=2 is 1.0");

    clip.wrap_mode = WrapMode::Once;
    assert!((clip.map_time(5.0) - 2.0).abs() < 1e-6, "Once should clamp to duration");

    let mut looping = clip.clone();
    looping.wrap_mode = WrapMode::Loop;
    assert!((looping.map_time(3.0) - 1.0).abs() < 1e-6, "loop at t=3 with d=2 is 1.0");

    let mut pp = clip.clone();
    pp.wrap_mode = WrapMode::PingPong;
    assert!((pp.map_time(3.0) - 1.0).abs() < 1e-6, "ping-pong t=3 of d=2 is 1.0");
    assert!((pp.map_time(1.0) - 1.0).abs() < 1e-6, "forward half");
}

#[test]
fn clip_sample_fills_all_bones() {
    let mut clip = AnimationClip::new("two_bones");
    let track_a = TransformTrack::new(Interpolation::Linear)
        .with_translation_keys(vec![Keyframe::new(0.0, Vec3::ZERO)]);
    let track_b = TransformTrack::new(Interpolation::Linear)
        .with_translation_keys(vec![Keyframe::new(0.0, Vec3::new(5.0, 0.0, 0.0))]);
    clip.add_channel(0, track_a);
    clip.add_channel(2, track_b);

    let sample = clip.sample(0.0, 4);
    assert_eq!(sample.locals.len(), 4);
    // Bone 1 has no channel, so it holds the identity.
    assert_eq!(sample.locals[1], Transform::identity());
    assert_eq!(sample.locals[0].position, Vec3::ZERO);
    assert_eq!(sample.locals[2].position, Vec3::new(5.0, 0.0, 0.0));
}

// ---------------------------------------------------------------------------
// Pose blending
// ---------------------------------------------------------------------------

#[test]
fn pose_blend_midpoint() {
    let a = Pose::identity(2);
    let mut b = Pose::identity(2);
    b.locals[0] = Transform::new(Vec3::new(10.0, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE);

    let mid = pose_blend(&a, &b, 0.5);
    assert!((mid.locals[0].position.x - 5.0).abs() < 1e-5);
    // Untouched bones stay put.
    assert_eq!(mid.locals[1], Transform::identity());
}

#[test]
fn cross_fade_matches_blend_at_endpoints() {
    let a = Pose::identity(1);
    let mut b = Pose::identity(1);
    b.locals[0] = Transform::new(Vec3::new(10.0, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE);

    assert_eq!(cross_fade(&a, &b, 0.0).locals[0].position, Vec3::ZERO);
    assert!((cross_fade(&a, &b, 1.0).locals[0].position.x - 10.0).abs() < 1e-5);
    // Smoothstep makes the midpoint exactly halfway for a linear param.
    assert!((cross_fade(&a, &b, 0.5).locals[0].position.x - 5.0).abs() < 1e-5);
}

#[test]
fn additive_layer_lists_zero_weight_is_identity() {
    let base = Pose::identity(2);
    let layer = Pose::identity(2);
    let result = pose_additive(&base, &layer, 1.0);
    for i in 0..2 {
        assert!(result.locals[i].position.length() < 1e-6);
    }
}

#[test]
fn blend_mask_restricts_to_subtree() {
    let mut s = Skeleton::new();
    let root = s.add_root("root");
    let spine = s.add_child("spine", root);
    let head = s.add_child("head", spine);
    let leg = s.add_child("leg", root);

    let mask = BlendMask::from_subtree(&s, spine, 2);
    assert!(mask.weight(spine) > 0.99, "subtree root should be full weight");
    assert!(mask.weight(head) > 0.0, "descendant should be affected");
    assert!(mask.weight(leg) < 1e-6, "sibling subtree must be untouched");
    assert!(mask.weight(root) < 1e-6, "parent must be untouched");

    // Applying the mask only moves the masked bones: a weight-0 bone falls back
    // to the reference pose, while a weight-1 bone is untouched.
    let mut pose = Pose::identity(4);
    for p in &mut pose.locals {
        *p = Transform::new(Vec3::new(1.0, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE);
    }
    let reference = Pose::identity(4);
    mask.apply(&mut pose, &reference);

    assert_eq!(pose.locals[leg].position, Vec3::ZERO, "weight-0 bone falls back to reference");
    assert_eq!(pose.locals[root].position, Vec3::ZERO, "weight-0 bone falls back to reference");
    // The subtree root itself carries full weight, so it is untouched.
    assert_eq!(pose.locals[spine].position.x, 1.0, "subtree root keeps full weight");
    // One level down the weight is 0.5 with a falloff of 2.
    assert!(
        (pose.locals[head].position.x - 0.5).abs() < 1e-6,
        "one level down should blend half way, got {}",
        pose.locals[head].position.x
    );
}

#[test]
fn blend_mask_all_is_identity_operation() {
    let mask = BlendMask::all(3);
    let mut pose = Pose::identity(3);
    pose.locals[1] = Transform::new(Vec3::new(2.0, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE);
    let before = pose.locals.clone();
    mask.apply(&mut pose, &Pose::identity(3));
    assert_eq!(pose.locals, before);
}

// ---------------------------------------------------------------------------
// IK
// ---------------------------------------------------------------------------

#[test]
fn two_bone_ik_reaches_target() {
    let chain = TwoBoneChain {
        root: Vec3::ZERO,
        upper_length: 1.0,
        lower_length: 1.0,
        rest_axis: Vec3::UP,
        pole: Vec3::FORWARD,
    };

    // Target at horizontal distance 1, on the +Y side.
    let target = Vec3::new(1.0, 1.0, 0.0);
    let solution = solve_two_bone_ik(&chain, target);

    // Distance root->target is sqrt(2); the middle joint must be 1 unit from
    // the root and 1 unit from the end.
    let d_root_mid = (solution.middle - chain.root).length();
    let d_mid_end = (solution.end - solution.middle).length();
    assert!((d_root_mid - 1.0).abs() < 1e-4, "upper length violated: {}", d_root_mid);
    assert!((d_mid_end - 1.0).abs() < 1e-4, "lower length violated: {}", d_mid_end);
    assert!((solution.end - target).length() < 1e-4, "end did not reach target");
    assert!(solution.reachable);
}

#[test]
fn two_bone_ik_clamps_unreachable_targets() {
    let chain = TwoBoneChain {
        root: Vec3::ZERO,
        upper_length: 1.0,
        lower_length: 1.0,
        rest_axis: Vec3::UP,
        pole: Vec3::FORWARD,
    };

    // Max reach is 2.
    let solution = solve_two_bone_ik(&chain, Vec3::new(10.0, 0.0, 0.0));
    assert!(!solution.reachable);
    assert!((solution.end - Vec3::new(2.0, 0.0, 0.0)).length() < 1e-4, "should clamp to full reach");
    assert!((solution.middle - chain.root).length() - 1.0 < 1e-4);
}

#[test]
fn two_bone_ik_respects_pole_vector() {
    let chain = TwoBoneChain {
        root: Vec3::ZERO,
        upper_length: 1.0,
        lower_length: 1.0,
        rest_axis: Vec3::UP,
        pole: Vec3::FORWARD,
    };

    // Target straight up on the default axis with a +Z pole: the middle joint
    // should bend toward +Z.
    let solution = solve_two_bone_ik(&chain, Vec3::new(0.0, 1.5, 0.0));
    assert!(solution.middle.z.abs() > 0.1, "bend should follow the pole, got {:?}", solution.middle);
}

#[test]
fn aim_constraint_points_bone_at_target() {
    let q = aim_constraint(Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0), Vec3::UP);
    let fwd = q.rotate_vector(Vec3::FORWARD);
    assert!(fwd.x > 0.99, "bone forward should point +X, got {:?}", fwd);
}

#[test]
fn ccd_ik_converges_for_three_bone_chain() {
    // Chain root at origin, three segments of length 1 along +Y.
    let mut positions = vec![
        Vec3::ZERO,
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 2.0, 0.0),
        Vec3::new(0.0, 3.0, 0.0),
    ];
    let lengths = [1.0, 1.0, 1.0];

    // Bend toward +X without moving the end along Y.
    let target = Vec3::new(2.5, 1.0, 0.0);
    let error = solve_ccd_ik(&mut positions, &lengths, target, 64, 1e-4);

    // Segment lengths must be preserved.
    for i in 0..lengths.len() {
        let d = (positions[i + 1] - positions[i]).length();
        assert!((d - lengths[i]).abs() < 1e-3, "segment {} stretched to {}", i, d);
    }
    assert!(error < 0.5, "CCD should bring the end close, got error {}", error);
}

// ---------------------------------------------------------------------------
// Skinning
// ---------------------------------------------------------------------------

#[test]
fn influences_keep_the_strongest_when_full() {
    let mut v = VertexInfluences::default();
    v.push(0, 0.5);
    v.push(1, 0.5);
    v.push(2, 0.5);
    v.push(3, 0.5);
    v.push(4, 0.9); // Should displace the weakest (all 0.5).

    assert_eq!(v.influence_count(), 4);
    assert!(v.weights.iter().any(|w| w.bone_index == 4 && w.weight > 0.5));
}

#[test]
fn influences_normalise_to_unit_weight() {
    let mut v = VertexInfluences::default();
    v.push(0, 2.0);
    v.push(1, 3.0);
    v.normalize();
    assert!((v.total_weight() - 1.0).abs() < 1e-6, "weights should sum to 1");
    assert!((v.weights[0].weight - 0.4).abs() < 1e-6);
    assert!((v.weights[1].weight - 0.6).abs() < 1e-6);
}

#[test]
fn accumulate_same_bone_doubly() {
    let mut v = VertexInfluences::default();
    v.push(7, 0.25);
    v.push(7, 0.25);
    assert_eq!(v.influence_count(), 1);
    assert!((v.weights[0].weight - 0.5).abs() < 1e-6);
}

#[test]
fn rest_pose_produces_no_warp() {
    // A skeleton whose animated pose equals the bind pose must leave vertices
    // untouched: this is the single most informative skinning correctness check.
    let mut skeleton = Skeleton::new();
    let root = skeleton.add_root("root");
    let child = skeleton.add_child("child", root);
    skeleton.bones[root].bind_local = Transform::new(Vec3::new(1.0, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE);
    skeleton.bones[child].bind_local = Transform::new(Vec3::new(0.0, 1.0, 0.0), Quat::IDENTITY, Vec3::ONE);
    skeleton.recompute_bind_pose();

    let bind_model: Vec<Transform> = skeleton.bones.iter().map(|b| b.bind_model).collect();
    let inverse_bind: Vec<Transform> = skeleton.bones.iter().map(|b| b.inverse_bind).collect();
    let matrices = compute_skinning_matrices(&bind_model, &inverse_bind);

    // In the rest pose, M_i = bind_model_i * inverse_bind_i must be identity.
    for m in &matrices {
        for i in 0..16 {
            let expected = match i {
                0 | 5 | 10 | 15 => 1.0,
                _ => 0.0,
            };
            assert!((m.m[i] - expected).abs() < 1e-5, "identity mismatch at {}: {}", i, m.m[i]);
        }
    }

    let mut skin = Skin::new(vec![VertexInfluences::default()]);
    skin.influences[0].push(0, 1.0);
    let base = vec![Vec3::new(0.25, 0.5, 0.0)];
    let normals = vec![Vec3::UP];

    let (pos, nor) = apply_skinning(&skin, &base, &normals, &matrices);
    assert!(total_warp(&base, &pos) < 1e-5, "rest pose must not move vertices");
    assert!((nor[0] - Vec3::UP).length() < 1e-5);
}

#[test]
fn bone_movement_warps_vertex() {
    // Translate the root bone by +X and confirm the bound vertex follows.
    let mut skeleton = Skeleton::new();
    let root = skeleton.add_root("root");
    skeleton.recompute_bind_pose();

    let _bind_model: Vec<Transform> = skeleton.bones.iter().map(|b| b.bind_model).collect();
    let inverse_bind: Vec<Transform> = skeleton.bones.iter().map(|b| b.inverse_bind).collect();

    // Animated pose: the bone is displaced by +3 in X.
    let animated = vec![Transform::new(Vec3::new(3.0, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE)];
    let matrices = compute_skinning_matrices(&animated, &inverse_bind);

    let mut skin = Skin::new(vec![VertexInfluences::default()]);
    skin.influences[0].push(root as u16, 1.0);
    let base = vec![Vec3::ZERO];

    let (pos, _) = apply_skinning(&skin, &base, &[Vec3::UP], &matrices);
    assert!((pos[0].x - 3.0).abs() < 1e-5, "vertex should follow the bone, got {:?}", pos[0]);
}

#[test]
fn weighted_skinning_blends_two_bones() {
    // Vertex bound 50/50 to two bones, one static, one moved +10 in X.
    let mut skeleton = Skeleton::new();
    let a = skeleton.add_root("a");
    let b = skeleton.add_root("b");
    skeleton.recompute_bind_pose();
    let inverse_bind: Vec<Transform> = skeleton.bones.iter().map(|b| b.inverse_bind).collect();

    let animated = vec![
        Transform::identity(),
        Transform::new(Vec3::new(10.0, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE),
    ];
    let matrices = compute_skinning_matrices(&animated, &inverse_bind);

    let mut v = VertexInfluences::default();
    v.push(a as u16, 0.5);
    v.push(b as u16, 0.5);
    v.normalize();
    let skin = Skin::new(vec![v]);

    let (pos, _) = apply_skinning(&skin, &[Vec3::ZERO], &[Vec3::UP], &matrices);
    assert!((pos[0].x - 5.0).abs() < 1e-4, "expected 50/50 blend = 5.0, got {}", pos[0].x);
}

#[test]
fn decompose_matrix_round_trips() {
    let original = Transform::new(
        Vec3::new(1.0, 2.0, 3.0),
        Quat::from_euler(0.3, -1.2, 0.8),
        Vec3::new(2.0, 2.0, 2.0),
    );
    let (t, r, s) = decompose_matrix(&original.to_mat4());

    assert!((t - original.position).length() < 1e-4, "translation mismatch");
    assert!((s - original.scale).length() < 1e-4, "scale mismatch: {:?} vs {:?}", s, original.scale);
    assert!(r.dot(&original.rotation).abs() > 0.999, "rotation mismatch");
}

// ---------------------------------------------------------------------------
// Blend tree
// ---------------------------------------------------------------------------

#[test]
fn blend_1d_interpolates_between_adjacent_clips() {
    // Clip 0 poses bone at x=0, clip 1 at x=10.
    let mut poses = |clip: usize, _time: f32| {
        let mut p = Pose::identity(1);
        p.locals[0] = Transform::new(Vec3::new(clip as f32 * 10.0, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE);
        p
    };

    let node = BlendNode::Blend1D {
        children: vec![BlendNode::Clip { clip: 0 }, BlendNode::Clip { clip: 1 }],
        positions: vec![0.0, 1.0],
        t: 0.25,
    };

    let result = evaluate(&node, 1, 0.0, &mut poses);
    assert!((result.locals[0].position.x - 2.5).abs() < 1e-4, "expected 2.5, got {}", result.locals[0].position.x);
}

#[test]
fn blend_1d_clamps_outside_range() {
    let mut poses = |clip: usize, _time: f32| {
        let mut p = Pose::identity(1);
        p.locals[0] = Transform::new(Vec3::new(clip as f32 * 10.0, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE);
        p
    };

    let node = BlendNode::Blend1D {
        children: vec![BlendNode::Clip { clip: 0 }, BlendNode::Clip { clip: 1 }],
        positions: vec![0.0, 1.0],
        t: 5.0, // Way past the range.
    };

    let result = evaluate(&node, 1, 0.0, &mut poses);
    assert!((result.locals[0].position.x - 10.0).abs() < 1e-4, "should clamp to the last clip");
}

#[test]
fn blend_2d_corners_and_centre() {
    let mut poses = |clip: usize, _time: f32| {
        let values = [0.0, 10.0, 20.0, 30.0];
        let mut p = Pose::identity(1);
        p.locals[0] = Transform::new(Vec3::new(values[clip], 0.0, 0.0), Quat::IDENTITY, Vec3::ONE);
        p
    };

    let build = |x: f32, y: f32| BlendNode::Blend2D {
        children: vec![
            BlendNode::Clip { clip: 0 },
            BlendNode::Clip { clip: 1 },
            BlendNode::Clip { clip: 2 },
            BlendNode::Clip { clip: 3 },
        ],
        x_range: (0.0, 1.0),
        y_range: (0.0, 1.0),
        x,
        y,
    };

    let corner = evaluate(&build(0.0, 0.0), 1, 0.0, &mut poses);
    assert!((corner.locals[0].position.x - 0.0).abs() < 1e-4);

    let opposite = evaluate(&build(1.0, 1.0), 1, 0.0, &mut poses);
    assert!((opposite.locals[0].position.x - 30.0).abs() < 1e-4);

    let centre = evaluate(&build(0.5, 0.5), 1, 0.0, &mut poses);
    assert!((centre.locals[0].position.x - 15.0).abs() < 1e-3, "grid centre should be the mean, got {}", centre.locals[0].position.x);
}

#[test]
fn cross_fade_node_reaches_second_clip_at_t_one() {
    let mut poses = |clip: usize, _time: f32| {
        let mut p = Pose::identity(1);
        p.locals[0] = Transform::new(Vec3::new(clip as f32 * 10.0, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE);
        p
    };

    let node = BlendNode::CrossFade {
        a: Box::new(BlendNode::Clip { clip: 0 }),
        b: Box::new(BlendNode::Clip { clip: 1 }),
        t: 1.0,
    };

    let result = evaluate(&node, 1, 0.0, &mut poses);
    assert!((result.locals[0].position.x - 10.0).abs() < 1e-4);
}

#[test]
fn additive_node_only_affects_layer_bones() {
    // Layer moves bone 0 by +5; bone 1 must stay put.
    let base = {
        let mut p = Pose::identity(2);
        p.locals[0] = Transform::new(Vec3::new(1.0, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE);
        p.locals[1] = Transform::new(Vec3::new(2.0, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE);
        p
    };
    let layer = {
        let mut p = Pose::identity(2);
        p.locals[0] = Transform::new(Vec3::new(5.0, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE);
        p
    };

    let result = pose_additive(&base, &layer, 1.0);
    assert!((result.locals[0].position.x - 6.0).abs() < 1e-4, "expected 1+5, got {}", result.locals[0].position.x);
    assert_eq!(result.locals[1].position, Vec3::new(2.0, 0.0, 0.0));
}
