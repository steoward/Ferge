use ow_math::Vec3;
use ow_physics::gjk::*;
use ow_physics::broadphase::{Aabb, SpatialHash, brute_force_pairs};
use std::collections::HashMap;

#[test]
fn separated_spheres_do_not_intersect() {
    let a = SphereSupport::new(Vec3::new(0.0, 0.0, 0.0), 1.0);
    let b = SphereSupport::new(Vec3::new(3.0, 0.0, 0.0), 1.0);

    assert!(!gjk_intersects(&a, &b));
    assert!(collide(&a, &b).is_none());

    let d = gjk_distance(&a, &b).unwrap();
    assert!((d - 1.0).abs() < 1e-3, "expected gap 1.0, got {}", d);
}

#[test]
fn overlapping_spheres_intersect_with_correct_depth() {
    let a = SphereSupport::new(Vec3::new(0.0, 0.0, 0.0), 1.0);
    let b = SphereSupport::new(Vec3::new(1.0, 0.0, 0.0), 1.0);

    assert!(gjk_intersects(&a, &b));
    let c = collide(&a, &b).expect("contact");
    // Overlap of two unit spheres 1 unit apart is exactly 1.0.
    assert!((c.depth - 1.0).abs() < 1e-2, "expected depth 1.0, got {}", c.depth);
    // Normal must point from A toward B (+X).
    assert!(c.normal.x > 0.9, "normal should point +X, got {:?}", c.normal);
}

#[test]
fn sphere_box_head_on_penetration() {
    let sphere = SphereSupport::new(Vec3::new(0.0, 0.0, 0.5), 1.0);
    let box_ = BoxSupport::new(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));

    assert!(gjk_intersects(&sphere, &box_));
    let c = collide(&sphere, &box_).expect("contact");
    // The sphere reaches 1.5 deep along +Z; the box face is at z=1.
    assert!(c.depth > 0.3, "expected meaningful depth, got {}", c.depth);
    assert!(c.normal.z.abs() > 0.5, "normal should be mostly +-Z, got {:?}", c.normal);
}

#[test]
fn deeply_penetrating_boxes_report_minimum_depth() {
    // Cube A spans x ∈ [-1, 1]; cube B (half-extent 1, offset 0.5) spans
    // x ∈ [-0.5, 1.5]. The Minkowski difference therefore spans
    // x ∈ [-2.5, 1.5], and the origin sits 1.5 from the near face — that is
    // the minimum translation distance, so 1.5 is the correct answer.
    let a = BoxSupport::new(Vec3::ZERO, Vec3::ONE);
    let b = BoxSupport::new(Vec3::new(0.5, 0.0, 0.0), Vec3::ONE);

    assert!(gjk_intersects(&a, &b));
    let c = collide(&a, &b).expect("contact");
    assert!((c.depth - 1.5).abs() < 5e-2, "expected 1.5, got {}", c.depth);
}

#[test]
fn exactly_touching_shapes_are_separated_or_zero_depth() {
    let a = SphereSupport::new(Vec3::new(0.0, 0.0, 0.0), 1.0);
    let b = SphereSupport::new(Vec3::new(2.0, 0.0, 0.0), 1.0);

    // At exactly 2 units apart they touch but do not overlap.
    let d = gjk_distance(&a, &b);
    match d {
        Some(dist) => assert!(dist < 1e-2, "expected near-zero distance, got {}", dist),
        None => {
            // Treated as intersecting; the depth should then be ~0.
            let c = collide(&a, &b).expect("contact");
            assert!(c.depth < 1e-2, "expected ~0 depth, got {}", c.depth);
        }
    }
}

#[test]
fn convex_hull_matches_box_support_projection() {
    // An 8-corner hull is a cube; along any axis its support projection must
    // match the exact box's. (The witness vertex itself differs — a cube's
    // farthest point along X is a corner, the box's is a face centre — so the
    // test compares the projection, which is what collision actually uses.)
    let hull = ConvexHull::unit_cube(Vec3::ZERO, 1.0);
    let box_ = BoxSupport::new(Vec3::ZERO, Vec3::ONE);

    for dir in [Vec3::RIGHT, Vec3::UP, Vec3::FORWARD, -Vec3::RIGHT, -Vec3::UP] {
        let h = hull.support(dir);
        let b = box_.support(dir);
        assert!(
            (h.dot(&dir) - b.dot(&dir)).abs() < 1e-4,
            "hull projection {} != box projection {} along {:?}",
            h.dot(&dir), b.dot(&dir), dir
        );
    }
}

#[test]
fn capsule_support_picks_correct_end() {
    let cap = CapsuleSupport::new(
        Vec3::new(0.0, -1.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        0.5,
    );
    let up = cap.support(Vec3::UP);
    let down = cap.support(-Vec3::UP);

    assert!((up.y - 1.5).abs() < 1e-4, "top cap should be 1.5, got {}", up.y);
    assert!((down.y + 1.5).abs() < 1e-4, "bottom cap should be -1.5, got {}", down.y);
}

#[test]
fn point_support_is_constant() {
    let p = PointSupport { point: Vec3::new(3.0, 4.0, 5.0) };
    for dir in [Vec3::RIGHT, Vec3::UP, Vec3::FORWARD] {
        assert_eq!(p.support(dir), p.point);
    }
}

#[test]
fn minkowski_support_equals_difference_of_supports() {
    let a = SphereSupport::new(Vec3::ZERO, 2.0);
    let b = SphereSupport::new(Vec3::new(5.0, 0.0, 0.0), 1.0);
    let dir = Vec3::RIGHT;

    let m = minkowski_support(&a, &b, dir);
    assert_eq!(m.vertex, a.support(dir) - b.support(-dir));
    assert_eq!(m.support_a, a.support(dir));
    assert_eq!(m.support_b, b.support(-dir));
}

#[test]
fn sphere_sphere_contact_normal_is_symmetric() {
    // Centres 1.0 apart with unit radii: a genuine 1.0 overlap.
    let a = SphereSupport::new(Vec3::ZERO, 1.0);
    let b = SphereSupport::new(Vec3::new(0.0, 1.0, 0.0), 1.0);

    let ab = collide(&a, &b).expect("contact");
    let ba = collide(&b, &a).expect("contact");

    // Depths are exact and symmetric.
    assert!((ab.depth - 1.0).abs() < 1e-3, "expected depth 1.0, got {}", ab.depth);
    assert!((ab.depth - ba.depth).abs() < 1e-3, "depths should match");

    // Normals point in opposite hemispheres: swapping the argument order
    // flips the normal (EPA resolves the axis to within sub-degree accuracy,
    // so the check is directional rather than bit-exact).
    assert!(
        ab.normal.dot(&ba.normal) < -0.99,
        "normals should be near-opposite: {:?} vs {:?}",
        ab.normal,
        ba.normal
    );
    assert!(ab.normal.y > 0.9, "a→b normal should point +Y, got {:?}", ab.normal);
    assert!(ba.normal.y < -0.9, "b→a normal should point -Y, got {:?}", ba.normal);
}

#[test]
fn box_box_offset_along_z_reports_z_normal() {
    let a = BoxSupport::new(Vec3::ZERO, Vec3::ONE);
    let b = BoxSupport::new(Vec3::new(0.0, 0.0, 1.5), Vec3::ONE);

    let c = collide(&a, &b).expect("contact");
    assert!(
        c.normal.z.abs() > 0.9,
        "normal should be mostly +-Z, got {:?}",
        c.normal
    );
    assert!((c.depth - 0.5).abs() < 5e-2, "expected ~0.5, got {}", c.depth);
}

#[test]
fn corner_case_corner_produces_contact() {
    // Boxes meeting only at a corner still overlap, so EPA must find a valid
    // (if shallow) penetration.
    let a = BoxSupport::new(Vec3::ZERO, Vec3::ONE);
    let b = BoxSupport::new(Vec3::new(1.1, 1.1, 0.0), Vec3::ONE);

    let c = collide(&a, &b).expect("contact");
    assert!(
        c.normal.length_squared() > 0.5,
        "normal should be well defined, got {:?}",
        c.normal
    );
    assert!(c.depth > 0.0, "corner overlap should have positive depth");
}

#[test]
fn large_offset_is_definitely_separated() {
    let a = SphereSupport::new(Vec3::ZERO, 1.0);
    let b = SphereSupport::new(Vec3::new(50.0, 0.0, 0.0), 1.0);

    assert!(!gjk_intersects(&a, &b));
    let d = gjk_distance(&a, &b).unwrap();
    assert!((d - 48.0).abs() < 1e-2, "expected 48, got {}", d);
}

#[test]
fn hull_vs_hull_intersection_detected() {
    let a = ConvexHull::unit_cube(Vec3::ZERO, 1.0);
    let b = ConvexHull::unit_cube(Vec3::new(0.3, 0.2, 0.1), 1.0);

    assert!(gjk_intersects(&a, &b));
    let c = collide(&a, &b).expect("contact");
    assert!(c.depth > 0.0);
    assert!(c.normal.length() > 0.5);
}

// ---------------------------------------------------------------------------
// Broadphase
// ---------------------------------------------------------------------------

#[test]
fn spatial_hash_insert_and_query() {
    let mut hash = SpatialHash::new(10.0);
    hash.insert(1, Aabb::new(Vec3::new(5.0, 5.0, 5.0), Vec3::new(6.0, 6.0, 6.0)));
    hash.insert(2, Aabb::new(Vec3::new(50.0, 50.0, 50.0), Vec3::new(51.0, 51.0, 51.0)));

    let near = hash.query(Vec3::ZERO);
    assert!(near.contains(&1));
    assert!(!near.contains(&2));
    assert_eq!(hash.body_count(), 2);
}

#[test]
fn spatial_hash_removes_body_completely() {
    let mut hash = SpatialHash::new(10.0);
    hash.insert(7, Aabb::new(Vec3::ZERO, Vec3::ONE));
    assert!(hash.query(Vec3::ZERO).contains(&7));

    hash.remove(7);
    assert!(!hash.query(Vec3::ZERO).contains(&7));
    assert_eq!(hash.body_count(), 0);
}

#[test]
fn spatial_hash_reinsert_updates_cells() {
    let mut hash = SpatialHash::new(10.0);
    hash.insert(3, Aabb::new(Vec3::ZERO, Vec3::ONE));
    // Reinserting the same id at a new position must not leave stale entries.
    hash.insert(3, Aabb::new(Vec3::new(100.0, 100.0, 100.0), Vec3::new(101.0, 101.0, 101.0)));

    assert!(!hash.query(Vec3::ZERO).contains(&3));
    assert!(hash.query(Vec3::new(100.0, 100.0, 100.0)).contains(&3));
    assert_eq!(hash.body_count(), 1);
}

#[test]
fn spatial_hash_matches_brute_force_pairs() {
    let mut aabbs: HashMap<u32, Aabb> = HashMap::new();
    aabbs.insert(1, Aabb::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 2.0, 2.0)));
    aabbs.insert(2, Aabb::new(Vec3::new(1.0, 1.0, 1.0), Vec3::new(3.0, 3.0, 3.0)));
    aabbs.insert(3, Aabb::new(Vec3::new(20.0, 20.0, 20.0), Vec3::new(21.0, 21.0, 21.0)));
    aabbs.insert(4, Aabb::new(Vec3::new(21.0, 21.0, 21.0), Vec3::new(22.0, 22.0, 22.0)));
    aabbs.insert(5, Aabb::new(Vec3::new(-20.0, 0.0, 0.0), Vec3::new(-19.0, 1.0, 1.0)));

    let brute = brute_force_pairs(&aabbs);
    assert_eq!(brute, vec![(1, 2), (3, 4)], "brute force reference");

    let mut hash = SpatialHash::new(5.0);
    for (id, b) in &aabbs {
        hash.insert(*id, *b);
    }

    let mut got = hash.candidate_pairs(&aabbs);
    got.sort_unstable();
    let mut expected = brute.clone();
    expected.sort_unstable();
    assert_eq!(got, expected, "spatial hash must match brute force");
}

#[test]
fn spatial_hash_finds_no_pairs_for_disjoint_cloud() {
    let mut aabbs: HashMap<u32, Aabb> = HashMap::new();
    for i in 0..50 {
        let base = (i as f32) * 100.0;
        aabbs.insert(i, Aabb::new(Vec3::new(base, base, base), Vec3::new(base + 1.0, base + 1.0, base + 1.0)));
    }

    let mut hash = SpatialHash::new(10.0);
    for (id, b) in &aabbs {
        hash.insert(*id, *b);
    }
    assert!(hash.candidate_pairs(&aabbs).is_empty());
}

#[test]
fn spatial_hash_scales_to_many_bodies() {
    // 2000 bodies on a 20x20x5 grid spaced 2.0 apart, each 2.5 units wide, so
    // every body overlaps its grid neighbours.
    let mut aabbs: HashMap<u32, Aabb> = HashMap::new();
    let mut id = 0u32;
    for x in 0..20 {
        for y in 0..20 {
            for z in 0..5 {
                let base = Vec3::new(x as f32 * 2.0, y as f32 * 2.0, z as f32 * 2.0);
                aabbs.insert(id, Aabb::new(base, base + Vec3::ONE * 2.5));
                id += 1;
            }
        }
    }

    let mut hash = SpatialHash::new(10.0);
    for (k, b) in &aabbs {
        hash.insert(*k, *b);
    }

    let pairs = hash.candidate_pairs(&aabbs);
    assert!(!pairs.is_empty(), "should find the genuine overlaps");

    // Correctness must hold: the found set equals the brute-force set.
    let mut brute = brute_force_pairs(&aabbs);
    brute.sort_unstable();
    let mut got = pairs.clone();
    got.sort_unstable();
    assert_eq!(got, brute);
}
