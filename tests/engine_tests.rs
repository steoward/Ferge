use ow_math::{Vec3, Quat, Mat4, Transform};

#[test]
fn vec3_basic_math() {
    let a = Vec3::new(1.0, 2.0, 3.0);
    let b = Vec3::new(4.0, 5.0, 6.0);

    assert_eq!(a + b, Vec3::new(5.0, 7.0, 9.0));
    assert_eq!(b - a, Vec3::new(3.0, 3.0, 3.0));
    assert_eq!(a * 2.0, Vec3::new(2.0, 4.0, 6.0));
    assert_eq!(-a, Vec3::new(-1.0, -2.0, -3.0));
}

#[test]
fn vec3_length_and_normalize() {
    let v = Vec3::new(3.0, 4.0, 0.0);
    assert_eq!(v.length(), 5.0);
    let n = v.normalize();
    assert!((n.length() - 1.0).abs() < 1e-6);
}

#[test]
fn vec3_dot_and_cross() {
    let x = Vec3::RIGHT;
    let y = Vec3::UP;
    let z = Vec3::FORWARD;

    assert_eq!(x.dot(&y), 0.0);
    assert_eq!(x.cross(y), z);
    assert_eq!(y.cross(z), x);
    assert_eq!(z.cross(x), y);
}

#[test]
fn vec3_lerp_interpolates() {
    let a = Vec3::ZERO;
    let b = Vec3::new(10.0, 20.0, 30.0);
    let mid = a.lerp(&b, 0.5);
    assert_eq!(mid, Vec3::new(5.0, 10.0, 15.0));
}

#[test]
fn quat_identity_leaves_vector_unchanged() {
    let v = Vec3::new(1.0, 0.0, 0.0);
    let r = Quat::IDENTITY.rotate_vector(v);
    assert!((r.x - v.x).abs() < 1e-6);
    assert!((r.y - v.y).abs() < 1e-6);
    assert!((r.z - v.z).abs() < 1e-6);
}

#[test]
fn quat_rotation_90_degrees_y() {
    let q = Quat::from_axis_angle(Vec3::UP, std::f32::consts::FRAC_PI_2);
    let r = q.rotate_vector(Vec3::FORWARD);
    assert!(r.x.abs() > 0.99, "expected rotate toward +X, got {:?}", r);
    assert!(r.y.abs() < 1e-6);
    assert!(r.z.abs() < 1e-6);
}

#[test]
fn quat_slerp_matches_endpoints() {
    let a = Quat::IDENTITY;
    let b = Quat::from_axis_angle(Vec3::UP, std::f32::consts::FRAC_PI_2);
    let start = a.slerp(&b, 0.0);
    let end = a.slerp(&b, 1.0);
    assert!((start.dot(&a)).abs() > 0.999);
    assert!((end.dot(&b)).abs() > 0.999);
}

#[test]
fn quat_normalization_and_conjugate() {
    let q = Quat::new(1.0, 2.0, 3.0, 4.0);
    let n = q.normalize();
    assert!((n.length() - 1.0).abs() < 1e-6);

    let c = q.conjugate();
    assert_eq!(c.x, -q.x);
    assert_eq!(c.w, q.w);

    let product = (q * c).normalize();
    assert!((product.dot(&Quat::IDENTITY)).abs() > 0.999);
}

#[test]
fn mat4_rotate_matches_quaternion_rotation() {
    // Regression test: building the rotation block row-major while the rest of
    // the matrix is column-major transposes R, which silently inverts the
    // rotation. This pins the convention down.
    let angle = std::f32::consts::FRAC_PI_2;
    let q = Quat::from_axis_angle(Vec3::UP, angle);

    let m = Mat4::identity().rotate(q);
    let via_matrix = m.transform_point(Vec3::FORWARD);
    let via_quat = q.rotate_vector(Vec3::FORWARD);

    assert!(
        (via_matrix - via_quat).length() < 1e-4,
        "matrix rotation {:?} must match quaternion rotation {:?}",
        via_matrix,
        via_quat
    );
    // A quarter turn about +Y sends +Z to +X.
    assert!((via_matrix.x).abs() > 0.99, "expected +X, got {:?}", via_matrix);
    assert!(via_matrix.z.abs() < 1e-4);
}

#[test]
fn mat4_rotate_composes_with_translate_and_scale() {
    // Translate, then rotate, then scale: the transform must move a point the
    // way Transform::to_mat4 (and hence the renderer) expects.
    let q = Quat::from_axis_angle(Vec3::UP, std::f32::consts::FRAC_PI_2);
    let m = Mat4::identity()
        .translate(Vec3::new(0.0, 10.0, 0.0))
        .rotate(q)
        .scale(Vec3::new(2.0, 1.0, 1.0));

    let p = m.transform_point(Vec3::new(1.0, 0.0, 0.0));
    // Scale first: (2, 0, 0); then rotate +90° about Y: (0, 0, -2); then translate.
    assert!(
        (p - Vec3::new(0.0, 10.0, -2.0)).length() < 1e-4,
        "expected (0, 10, -2), got {:?}",
        p
    );
}

#[test]
fn mat4_identity_transforms_points() {
    let m = Mat4::identity();
    let p = Vec3::new(5.0, 6.0, 7.0);
    assert_eq!(m.transform_point(p), p);
}

#[test]
fn mat4_translation_moves_points() {
    let m = Mat4::identity().translate(Vec3::new(1.0, 2.0, 3.0));
    let p = m.transform_point(Vec3::ZERO);
    assert_eq!(p, Vec3::new(1.0, 2.0, 3.0));
}

#[test]
fn mat4_scale_scales_points() {
    let m = Mat4::identity().scale(Vec3::new(2.0, 3.0, 4.0));
    let p = m.transform_point(Vec3::new(1.0, 1.0, 1.0));
    assert_eq!(p, Vec3::new(2.0, 3.0, 4.0));
}

#[test]
fn mat4_perspective_produces_valid_matrix() {
    let m = Mat4::perspective(std::f32::consts::FRAC_PI_4, 1.777, 0.1, 1000.0);
    assert!(m.m[0] > 0.0);
    assert_eq!(m.m[11], -1.0);
}

#[test]
fn mat4_look_at_orients_basis() {
    let m = Mat4::look_at(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, Vec3::UP);
    let origin = m.transform_point(Vec3::new(0.0, 0.0, 10.0));
    assert!(origin.z.abs() < 1e-5, "eye point must map to origin, got {:?}", origin);
}

#[test]
fn transform_composition() {
    let t = Transform::new(
        Vec3::new(1.0, 0.0, 0.0),
        Quat::from_axis_angle(Vec3::UP, std::f32::consts::FRAC_PI_2),
        Vec3::ONE,
    );
    let fwd = t.forward();
    assert!(fwd.x.abs() > 0.99, "forward should be +X after 90deg yaw, got {:?}", fwd);

    let mat = t.to_mat4();
    let origin = mat.transform_point(Vec3::ZERO);
    assert_eq!(origin, Vec3::new(1.0, 0.0, 0.0));
}

#[test]
fn transform_lerp_blends() {
    let a = Transform::identity();
    let b = Transform::new(Vec3::new(10.0, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE);
    let mid = a.lerp(&b, 0.5);
    assert!((mid.position.x - 5.0).abs() < 1e-6);
}

#[test]
fn mat4_transpose_involution() {
    let m = Mat4::identity().translate(Vec3::new(3.0, 4.0, 5.0));
    let t = m.transpose().transpose();
    for i in 0..16 {
        assert!((t.m[i] - m.m[i]).abs() < 1e-6);
    }
}

#[test]
fn mat4_matrix_multiplication_associates() {
    let a = Mat4::identity().translate(Vec3::new(1.0, 0.0, 0.0));
    let b = Mat4::identity().scale(Vec3::new(2.0, 2.0, 2.0));
    let c = Mat4::identity().translate(Vec3::new(0.0, 3.0, 0.0));

    let left = (a * b) * c;
    let right = a * (b * c);
    for i in 0..16 {
        assert!((left.m[i] - right.m[i]).abs() < 1e-6);
    }
}
