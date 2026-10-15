//! Convex narrowphase collision detection.
//!
//! Implements the Gilbert-Johnson-Keerthi (GJK) algorithm for intersection
//! testing and the Expanding Polytope Algorithm (EPA) for penetration depth
//! and contact normal.
//!
//! Shapes are described only by their support mapping — the farthest point in a
//! given direction. This is the minimal information GJK/EPA require, so any
//! convex shape (sphere, box, capsule, point cloud, convex hull) can be used.

use ow_math::Vec3;

/// 1/√3, defined locally because `f32::consts::FRAC_1_SQRT_3` is still unstable
/// on this toolchain. Used only to build the seed spread directions, so the
/// extra digits buy nothing — the exact value is recomputed where it matters.
#[allow(clippy::approx_constant)]
const FRAC_1_SQRT_3: f32 = 0.577_350_26;

/// Maximum simplex evolution iterations before giving up.
pub const GJK_MAX_ITERATIONS: usize = 64;

/// Maximum polytope expansion iterations before giving up. EPA normally
/// converges in a handful of steps; the budget is generous to absorb shapes
/// with many faces.
pub const EPA_MAX_ITERATIONS: usize = 256;

/// Convergence threshold for EPA.
pub const EPA_TOLERANCE: f32 = 1e-4;

/// Number of support directions tried when rebuilding a degenerate seed.
const SEED_SPREAD_LEN: usize = 14;

/// A point in the Minkowski difference space, remembering which support point
/// of each shape produced it. Retaining the witness points is what lets EPA
/// reconstruct a contact normal without re-running the support function.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SupportPoint {
    /// `support_a(direction) - support_b(-direction)`
    pub vertex: Vec3,
    /// Witness point on shape A.
    pub support_a: Vec3,
    /// Witness point on shape B.
    pub support_b: Vec3,
}

impl SupportPoint {
    pub fn new(vertex: Vec3, support_a: Vec3, support_b: Vec3) -> Self {
        Self {
            vertex,
            support_a,
            support_b,
        }
    }
}

/// Something that can answer "what is the farthest point you have in this
/// direction?". This is the only contract GJK/EPA need.
pub trait SupportGeometry {
    fn support(&self, direction: Vec3) -> Vec3;
}

/// Spherical support: the farthest point is the centre plus the radius along
/// the direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SphereSupport {
    pub center: Vec3,
    pub radius: f32,
}

impl SphereSupport {
    pub fn new(center: Vec3, radius: f32) -> Self {
        Self { center, radius }
    }
}

impl SupportGeometry for SphereSupport {
    fn support(&self, direction: Vec3) -> Vec3 {
        let d = direction.normalize();
        self.center + Vec3::new(d.x * self.radius, d.y * self.radius, d.z * self.radius)
    }
}

/// A convex hull stored as a point cloud. The support function scans every
/// vertex — O(n) but exact, and adequate for hulls up to a few thousand
/// vertices (real hulls are far smaller, typically < 64 faces).
#[derive(Debug, Clone, PartialEq)]
pub struct ConvexHull {
    pub center: Vec3,
    pub points: Vec<Vec3>,
}

impl ConvexHull {
    /// Builds a hull from world-space points.
    pub fn new(points: Vec<Vec3>) -> Self {
        let center = if points.is_empty() {
            Vec3::ZERO
        } else {
            let mut c = Vec3::ZERO;
            for p in &points {
                c = c + *p;
            }
            Vec3::new(
                c.x / points.len() as f32,
                c.y / points.len() as f32,
                c.z / points.len() as f32,
            )
        };
        Self { center, points }
    }

    /// Builds a cube centred at the origin with the given half-extent — handy
    /// for tests and for box colliders that need a hull representation.
    pub fn unit_cube(center: Vec3, half: f32) -> Self {
        let h = half.abs();
        Self::new(vec![
            center + Vec3::new(-h, -h, -h),
            center + Vec3::new(h, -h, -h),
            center + Vec3::new(-h, h, -h),
            center + Vec3::new(h, h, -h),
            center + Vec3::new(-h, -h, h),
            center + Vec3::new(h, -h, h),
            center + Vec3::new(-h, h, h),
            center + Vec3::new(h, h, h),
        ])
    }

    pub fn vertex_count(&self) -> usize {
        self.points.len()
    }

    /// Radius of the smallest sphere centred at the centroid containing every
    /// vertex. Used as a conservative bounding radius for early rejection.
    pub fn bounding_radius(&self) -> f32 {
        self.points
            .iter()
            .map(|p| (*p - self.center).length())
            .fold(0.0f32, f32::max)
    }
}

impl SupportGeometry for ConvexHull {
    fn support(&self, direction: Vec3) -> Vec3 {
        if self.points.is_empty() {
            return self.center;
        }
        let mut best = self.points[0];
        let mut best_dot = best.dot(&direction);
        for p in &self.points[1..] {
            let d = p.dot(&direction);
            if d > best_dot {
                best_dot = d;
                best = *p;
            }
        }
        best
    }
}

/// An axis-aligned box. Its support point is exact and O(1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxSupport {
    pub center: Vec3,
    pub half_extents: Vec3,
}

impl BoxSupport {
    pub fn new(center: Vec3, half_extents: Vec3) -> Self {
        Self {
            center,
            half_extents: Vec3::new(
                half_extents.x.abs(),
                half_extents.y.abs(),
                half_extents.z.abs(),
            ),
        }
    }
}

impl SupportGeometry for BoxSupport {
    fn support(&self, direction: Vec3) -> Vec3 {
        let s = Vec3::new(
            if direction.x >= 0.0 {
                self.half_extents.x
            } else {
                -self.half_extents.x
            },
            if direction.y >= 0.0 {
                self.half_extents.y
            } else {
                -self.half_extents.y
            },
            if direction.z >= 0.0 {
                self.half_extents.z
            } else {
                -self.half_extents.z
            },
        );
        self.center + s
    }
}

/// A capsule as two hemispheres joined by a cylinder.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CapsuleSupport {
    pub point_a: Vec3,
    pub point_b: Vec3,
    pub radius: f32,
}

impl CapsuleSupport {
    pub fn new(point_a: Vec3, point_b: Vec3, radius: f32) -> Self {
        Self {
            point_a,
            point_b,
            radius,
        }
    }
}

impl SupportGeometry for CapsuleSupport {
    fn support(&self, direction: Vec3) -> Vec3 {
        let d = direction.normalize();
        let r = self.radius.abs();
        let a = self.point_a + d * r;
        let b = self.point_b + d * r;
        if a.dot(&direction) >= b.dot(&direction) {
            a
        } else {
            b
        }
    }
}

/// A degenerate single point. Useful as an immutable body type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointSupport {
    pub point: Vec3,
}

impl SupportGeometry for PointSupport {
    fn support(&self, _direction: Vec3) -> Vec3 {
        self.point
    }
}

/// Computes the support point of the Minkowski difference `A - B` along
/// `direction`: the farthest point of A along `direction` paired with the
/// farthest point of B along `-direction`.
pub fn minkowski_support(
    a: &impl SupportGeometry,
    b: &impl SupportGeometry,
    direction: Vec3,
) -> SupportPoint {
    let sa = a.support(direction);
    let sb = b.support(-direction);
    SupportPoint::new(Vec3::new(sa.x - sb.x, sa.y - sb.y, sa.z - sb.z), sa, sb)
}

/// `a × b × c` — the vector perpendicular to `c` inside the plane of `a, b`,
/// pointing in the direction needed for GJK's perpendicular-ray query.
fn triple_product(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    a.cross(b).cross(c)
}

/// Any non-zero vector perpendicular to `v`. Used to keep GJK growing when the
/// exact perpendicular is degenerate (the origin lies exactly on the current
/// feature), which would otherwise stall the simplex as a point, line or
/// triangle and leave EPA without the tetrahedron it needs.
fn any_perpendicular(v: Vec3) -> Vec3 {
    let axis = if v.x.abs() < v.z.abs() {
        Vec3::RIGHT
    } else {
        Vec3::FORWARD
    };
    let n = v.cross(axis);
    if n.length_squared() > 1e-12 {
        return n;
    }
    let n = v.cross(Vec3::UP);
    if n.length_squared() > 1e-12 {
        return n;
    }
    v.cross(Vec3::RIGHT)
}

/// The exact perpendicular to `v` toward the origin, falling back to an
/// arbitrary perpendicular when the triple product degenerates.
fn perpendicular_toward(v: Vec3, ao: Vec3) -> Option<Vec3> {
    let exact = triple_product(v, ao, v);
    if exact.length_squared() > 1e-12 {
        return Some(exact);
    }
    let any = any_perpendicular(v);
    if any.length_squared() > 1e-12 {
        return Some(any);
    }
    None
}

/// A growing simplex of up to four points. Index 0 always holds the most
/// recently added point, which is the convention the case analysis expects.
#[derive(Debug, Clone, Copy)]
pub struct Simplex {
    points: [SupportPoint; 4],
    count: usize,
}

impl Simplex {
    pub fn new() -> Self {
        let zero = SupportPoint::new(Vec3::ZERO, Vec3::ZERO, Vec3::ZERO);
        Self {
            points: [zero; 4],
            count: 0,
        }
    }

    pub fn count(&self) -> usize {
        self.count
    }

    /// Inserts `p` at index 0, shifting existing points up.
    pub fn push_front(&mut self, p: SupportPoint) {
        if self.count >= 4 {
            self.count = 3;
        }
        for i in (1..=self.count).rev() {
            self.points[i] = self.points[i - 1];
        }
        self.points[0] = p;
        if self.count < 4 {
            self.count += 1;
        }
    }
}

impl Default for Simplex {
    fn default() -> Self {
        Self::new()
    }
}

/// Outcome of a simplex evolution step.
#[derive(Debug, Clone, Copy, PartialEq)]
enum GjkStep {
    /// Search in this direction next.
    Search(Vec3),
    /// The simplex encloses the origin: the shapes intersect.
    Enclosed,
    /// Could not make progress.
    Stalled,
}

/// Expands the simplex toward the origin, shrinking it back to the closest
/// feature when the origin lies outside. Returns true when enclosed.
fn evolve_simplex(simplex: &mut Simplex) -> GjkStep {
    match simplex.count {
        2 => line_case(simplex),
        3 => triangle_case(simplex),
        4 => tetrahedron_case(simplex),
        _ => GjkStep::Stalled,
    }
}

/// Two-point case: the origin is either past A (toward B) or closer to B.
fn line_case(simplex: &mut Simplex) -> GjkStep {
    let a = simplex.points[0].vertex;
    let b = simplex.points[1].vertex;
    let ab = b - a;
    let ao = -a;

    if ab.dot(&ao) > 0.0 {
        // Closest feature is the segment AB; search perpendicular to it, in
        // the plane spanned by the origin.
        match perpendicular_toward(ab, ao) {
            Some(dir) => GjkStep::Search(dir),
            None => GjkStep::Enclosed,
        }
    } else {
        // Closest feature is B; drop A.
        simplex.points[0] = simplex.points[1];
        simplex.count = 1;
        GjkStep::Search(ao)
    }
}

/// Three-point case: the origin projects onto the triangle, one of its edges,
/// or A itself.
fn triangle_case(simplex: &mut Simplex) -> GjkStep {
    let a = simplex.points[0].vertex;
    let b = simplex.points[1].vertex;
    let c = simplex.points[2].vertex;

    let ab = b - a;
    let ac = c - a;
    let ao = -a;
    let abc = ab.cross(ac);

    // Outside edge AC (checked first by convention)?
    if abc.cross(ac).dot(&ao) > 0.0 {
        if ac.dot(&ao) > 0.0 {
            // Closest feature is edge AC: keep A and C.
            simplex.points[1] = simplex.points[2];
            simplex.count = 2;
            return match perpendicular_toward(ac, ao) {
                Some(dir) => GjkStep::Search(dir),
                None => GjkStep::Enclosed,
            };
        }
        // Closest feature is A.
        simplex.count = 1;
        return GjkStep::Search(ao);
    }

    // Outside edge AB?
    if ab.cross(abc).dot(&ao) > 0.0 {
        if ab.dot(&ao) > 0.0 {
            // Closest feature is edge AB: keep A and B.
            simplex.points[2] = simplex.points[1];
            simplex.count = 2;
            return match perpendicular_toward(ab, ao) {
                Some(dir) => GjkStep::Search(dir),
                None => GjkStep::Enclosed,
            };
        }
        simplex.count = 1;
        return GjkStep::Search(ao);
    }

    // The origin projects onto the triangle's interior, above or below.
    if abc.dot(&ao) > 0.0 {
        // Above the triangle: the closest feature is the triangle itself.
        GjkStep::Search(abc)
    } else {
        // Below: flip the winding so the normal faces the origin.
        simplex.points.swap(1, 2);
        GjkStep::Search(-abc)
    }
}

/// Four-point case: the origin is outside one of the three faces, or inside.
fn tetrahedron_case(simplex: &mut Simplex) -> GjkStep {
    let a = simplex.points[0].vertex;
    let b = simplex.points[1].vertex;
    let c = simplex.points[2].vertex;
    let d = simplex.points[3].vertex;

    let ao = -a;
    let ab = b - a;
    let ac = c - a;
    let ad = d - a;

    let abc = ab.cross(ac);
    let acd = ac.cross(ad);
    let adb = ad.cross(ab);

    if abc.dot(&ao) > 0.0 {
        // Outside face ABC: keep A, B, C.
        simplex.count = 3;
        return triangle_case(simplex);
    }
    if acd.dot(&ao) > 0.0 {
        // Outside face ACD: keep A, C, D.
        simplex.points[1] = simplex.points[2];
        simplex.points[2] = simplex.points[3];
        simplex.count = 3;
        return triangle_case(simplex);
    }
    if adb.dot(&ao) > 0.0 {
        // Outside face ADB: keep A, D, B.
        simplex.points[2] = simplex.points[1];
        simplex.points[1] = simplex.points[3];
        simplex.count = 3;
        return triangle_case(simplex);
    }

    // The tetrahedron encloses the origin.
    GjkStep::Enclosed
}

/// Result of a GJK query.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GjkResult {
    /// Shapes are separated by `distance`.
    Separated { distance: f32 },
    /// Shapes intersect.
    Intersecting,
    /// The iteration budget was exhausted; the answer is unknown.
    Exhausted,
}

/// Interior-point estimate used to seed the first search direction. Averaging
/// support points over the basis vectors yields a point inside any convex set.
fn interior_point(g: &impl SupportGeometry) -> Vec3 {
    let mut sum = Vec3::ZERO;
    let basis = [Vec3::RIGHT, Vec3::UP, Vec3::FORWARD];
    for d in basis {
        let s = g.support(d);
        sum = sum + s;
    }
    Vec3::new(sum.x / 3.0, sum.y / 3.0, sum.z / 3.0)
}

/// Runs GJK, returning whether the shapes are separated, intersecting, or
/// inconclusive. The witness points of the final simplex are kept so callers
/// can derive a contact.
pub fn gjk<A: SupportGeometry, B: SupportGeometry>(a: &A, b: &B) -> (GjkResult, Simplex) {
    // Seed the direction along the line between the two shapes.
    let mut direction = interior_point(b) - interior_point(a);
    if direction.length_squared() < 1e-12 {
        direction = Vec3::RIGHT;
    }

    let mut simplex = Simplex::new();
    simplex.push_front(minkowski_support(a, b, direction));
    direction = -simplex.points[0].vertex;

    // In a symmetric touching configuration the first support point can land
    // exactly on the origin, which zeroes the search direction. That is not
    // evidence of intersection — re-seed along a perpendicular instead.
    if direction.length_squared() < 1e-12 {
        direction = any_perpendicular(Vec3::RIGHT);
        if direction.length_squared() < 1e-12 {
            direction = Vec3::UP;
        }
    }

    for _ in 0..GJK_MAX_ITERATIONS {
        if direction.length_squared() < 1e-12 {
            // Only conclude enclosure once the simplex has enough points to
            // actually contain the origin; otherwise re-seed and keep going.
            if simplex.count() >= 4 {
                return (GjkResult::Intersecting, simplex);
            }
            direction = any_perpendicular(simplex.points[0].vertex);
            if direction.length_squared() < 1e-12 {
                direction = Vec3::UP;
            }
        }

        let p = minkowski_support(a, b, direction);

        // If the new support point does not pass the origin along the search
        // direction, the origin cannot be inside the Minkowski difference.
        if p.vertex.dot(&direction) <= 0.0 {
            return (GjkResult::Separated { distance: direction.length() }, simplex);
        }

        simplex.push_front(p);

        match evolve_simplex(&mut simplex) {
            GjkStep::Enclosed => return (GjkResult::Intersecting, simplex),
            GjkStep::Search(d) => {
                if d.length_squared() < 1e-12 {
                    return (GjkResult::Intersecting, simplex);
                }
                direction = d;
            }
            GjkStep::Stalled => return (GjkResult::Exhausted, simplex),
        }
    }

    (GjkResult::Exhausted, simplex)
}

/// Whether two convex shapes intersect.
pub fn gjk_intersects<A: SupportGeometry, B: SupportGeometry>(a: &A, b: &B) -> bool {
    matches!(gjk(a, b).0, GjkResult::Intersecting)
}

/// Separation distance between two convex shapes, or `None` when they
/// intersect or the query is inconclusive.
pub fn gjk_distance<A: SupportGeometry, B: SupportGeometry>(a: &A, b: &B) -> Option<f32> {
    match gjk(a, b).0 {
        GjkResult::Separated { distance } => Some(distance),
        _ => None,
    }
}

/// A triangle face of the EPA polytope.
#[derive(Debug, Clone, Copy, PartialEq)]
struct EpaFace {
    a: usize,
    b: usize,
    c: usize,
    normal: Vec3,
    distance: f32,
}

/// Face winding patterns of a tetrahedron given four vertex indices.
const TETRA_FACES: [(usize, usize, usize); 4] = [(0, 1, 2), (0, 2, 3), (0, 3, 1), (1, 3, 2)];

/// Support directions used to rebuild a degenerate seed. The first four are
/// the vertices of a regular tetrahedron, deliberately not axis-aligned so no
/// face plane can pass through the origin — the failure mode of an
/// axis-aligned seed on symmetric shapes.
const SEED_SPREAD: [Vec3; 14] = [
    Vec3::new(FRAC_1_SQRT_3, FRAC_1_SQRT_3, FRAC_1_SQRT_3),
    Vec3::new(FRAC_1_SQRT_3, -FRAC_1_SQRT_3, -FRAC_1_SQRT_3),
    Vec3::new(-FRAC_1_SQRT_3, FRAC_1_SQRT_3, -FRAC_1_SQRT_3),
    Vec3::new(-FRAC_1_SQRT_3, -FRAC_1_SQRT_3, FRAC_1_SQRT_3),
    Vec3::new(1.0, 0.0, 0.0),
    Vec3::new(0.0, 1.0, 0.0),
    Vec3::new(0.0, 0.0, 1.0),
    Vec3::new(-1.0, 0.0, 0.0),
    Vec3::new(0.0, -1.0, 0.0),
    Vec3::new(0.0, 0.0, -1.0),
    Vec3::new(std::f32::consts::FRAC_1_SQRT_2, std::f32::consts::FRAC_1_SQRT_2, 0.0),
    Vec3::new(-std::f32::consts::FRAC_1_SQRT_2, std::f32::consts::FRAC_1_SQRT_2, 0.0),
    Vec3::new(0.0, std::f32::consts::FRAC_1_SQRT_2, std::f32::consts::FRAC_1_SQRT_2),Vec3::new(FRAC_1_SQRT_3, FRAC_1_SQRT_3, -FRAC_1_SQRT_3),
];

/// A convex polytope: vertices plus consistently-wound triangular faces, all
/// oriented so their normals point away from the origin.
struct Polytope {
    verts: Vec<SupportPoint>,
    faces: Vec<EpaFace>,
}

impl Polytope {
    /// Builds a tetrahedron from exactly four vertices. Returns `None` if the
    /// points are degenerate (fewer than four, or a zero-area face).
    fn from_vertices(verts: Vec<SupportPoint>) -> Option<Self> {
        if verts.len() < 4 {
            return None;
        }

        let mut faces: Vec<EpaFace> = Vec::with_capacity(4);
        for (a, b, c) in TETRA_FACES {
            let normal = face_normal_outward(&verts, a, b, c)?;
            let distance = normal.dot(&verts[a].vertex);
            faces.push(EpaFace {
                a,
                b,
                c,
                normal,
                distance,
            });
        }

        Some(Self { verts, faces })
    }

    /// Whether the polytope is geometrically usable: faces exist, none is
    /// zero-area, and every face is a real distance from the origin.
    fn is_valid(&self) -> bool {
        !self.faces.is_empty()
            && self.faces.iter().all(|f| f.normal.length() > 1e-4)
    }

    /// Whether the origin is strictly inside: every face a positive distance
    /// away. This is the property EPA's main loop needs, and it is repaired by
    /// the growth phase when the seed is degenerate.
    fn encloses_origin(&self) -> bool {
        !self.faces.is_empty() && self.faces.iter().all(|f| f.distance > EPA_TOLERANCE)
    }

    /// Finds the face closest to the origin, returning its index and distance.
    fn closest_face(&self) -> (usize, f32) {
        let mut best = 0usize;
        let mut best_d = f32::INFINITY;
        for (i, f) in self.faces.iter().enumerate() {
            if f.distance < best_d {
                best_d = f.distance;
                best = i;
            }
        }
        (best, best_d)
    }

    /// Removes every face visible from the support point taken along face
    /// `i`'s normal, and stitches the resulting hole. Returns `false` when
    /// nothing could be expanded.
    fn expand<A: SupportGeometry, B: SupportGeometry>(
        &mut self,
        a: &A,
        b: &B,
        i: usize,
    ) -> bool {
        let normal = self.faces[i].normal;
        let new_point = minkowski_support(a, b, normal);

        let must_grow = new_point.vertex.dot(&normal) - self.faces[i].distance > EPA_TOLERANCE;
        if !must_grow {
            return false;
        }

        let previous_faces = self.faces.clone();
        let mut kept: Vec<EpaFace> = Vec::with_capacity(self.faces.len());
        let mut removed: Vec<EpaFace> = Vec::new();
        for f in self.faces.drain(..) {
            let to_point = new_point.vertex - self.verts[f.a].vertex;
            if to_point.dot(&f.normal) > 0.0 {
                removed.push(f);
            } else {
                kept.push(f);
            }
        }

        if removed.is_empty() {
            self.faces = previous_faces;
            return false;
        }

        // Boundary edges are directed edges of removed faces whose reverse
        // direction does not also belong to a removed face. A consistently
        // wound closed surface has each shared interior edge once in each
        // direction, so boundary edges appear exactly once.
        let mut boundary: Vec<(usize, usize)> = Vec::new();
        for f in &removed {
            for (from, to) in [(f.a, f.b), (f.b, f.c), (f.c, f.a)] {
                let shared = removed.iter().any(|g| {
                    (g.a == to && g.b == from) || (g.b == to && g.c == from) || (g.c == to && g.a == from)
                });
                if !shared && !boundary.contains(&(from, to)) {
                    boundary.push((from, to));
                }
            }
        }

        let new_index = self.verts.len();
        self.verts.push(new_point);

        // The new face must traverse the shared boundary edge in the opposite
        // direction to the surviving neighbour that already owns it. If the
        // removed face traversed it `from -> to`, the new face must traverse
        // it `to -> from`, so the new triangle is wound `(to, N, from)`.
        for (from, to) in boundary {
            if from == new_index || to == new_index {
                continue;
            }
            match face_normal_outward(&self.verts, to, new_index, from) {
                Some(n) => kept.push(EpaFace {
                    a: to,
                    b: new_index,
                    c: from,
                    distance: n.dot(&self.verts[to].vertex),
                    normal: n,
                }),
                None => continue,
            }
        }

        // Safety net: a valid closed polytope always has at least four faces.
        // If stitching produced fewer, the configuration was degenerate —
        // restore the previous surface and report failure rather than risk a
        // bogus contact from a broken mesh.
        if kept.len() < 4 {
            self.faces = previous_faces;
            return false;
        }

        self.faces = kept;
        true
    }
}

impl EpaFace {
    /// Average of the three witness points on shape A for this face.
    fn centroid_a(&self, verts: &[SupportPoint]) -> Vec3 {
        (verts[self.a].support_a + verts[self.b].support_a + verts[self.c].support_a) / 3.0
    }

    /// Average of the three witness points on shape B for this face.
    fn centroid_b(&self, verts: &[SupportPoint]) -> Vec3 {
        (verts[self.a].support_b + verts[self.b].support_b + verts[self.c].support_b) / 3.0
    }
}

/// Outward unit normal of the triangle `(a, b, c)`, oriented so it points away
/// from the origin. Returns `None` for a zero-area triangle.
fn face_normal_outward(verts: &[SupportPoint], a: usize, b: usize, c: usize) -> Option<Vec3> {
    let va = verts[a].vertex;
    let n = (verts[b].vertex - va).cross(verts[c].vertex - va);
    if n.length_squared() < 1e-12 {
        return None;
    }
    let n = n.normalize();
    if n.dot(&va) < 0.0 {
        Some(-n)
    } else {
        Some(n)
    }
}

/// Builds a valid seed polytope for EPA. Prefers the GJK simplex when it is
/// non-degenerate, otherwise rebuilds from spread support directions.
fn build_seed_polytope<A: SupportGeometry, B: SupportGeometry>(
    a: &A,
    b: &B,
    seed: &Simplex,
) -> Option<Polytope> {
    let from_simplex = Polytope::from_vertices(
        (0..seed.count())
            .map(|i| SupportPoint {
                vertex: seed.points[i].vertex,
                support_a: seed.points[i].support_a,
                support_b: seed.points[i].support_b,
            })
            .collect(),
    );
    if let Some(p) = from_simplex {
        if p.is_valid() {
            return Some(p);
        }
    }

    // Rebuild from spread directions, deduplicating as we go.
    let mut distinct: Vec<SupportPoint> = Vec::new();
    for d in SEED_SPREAD.iter().take(SEED_SPREAD_LEN) {
        if distinct.len() >= 4 {
            break;
        }
        let p = minkowski_support(a, b, *d);
        if distinct
            .iter()
            .all(|q| (q.vertex - p.vertex).length() > 1e-5)
        {
            distinct.push(p);
        }
    }

    let p = Polytope::from_vertices(distinct.clone());
    if p.is_some() && p.as_ref().unwrap().is_valid() {
        p
    } else {
        None
    }
}

/// Expands the polytope seeded by an intersecting GJK simplex until the face
/// closest to the origin is found. That face yields the penetration depth and
/// contact normal.
///
/// Returns `None` if a non-degenerate seed cannot be built.
pub fn epa_penetration<A: SupportGeometry, B: SupportGeometry>(
    a: &A,
    b: &B,
    seed: &Simplex,
) -> Option<Contact> {
    let mut polytope = build_seed_polytope(a, b, seed)?;

    // Growth phase: the origin must be strictly inside the polytope, otherwise
    // EPA has no well-defined closest face. Faces that are at or behind the
    // origin get pushed outward until the origin is enclosed.
    for _ in 0..EPA_MAX_ITERATIONS {
        if polytope.faces.is_empty() {
            return None;
        }
        if polytope.encloses_origin() {
            break;
        }
        let (i, _) = polytope.closest_face();
        if !polytope.expand(a, b, i) {
            break;
        }
    }

    // Main expansion phase.
    for _ in 0..EPA_MAX_ITERATIONS {
        if polytope.faces.is_empty() {
            return None;
        }
        let (closest_i, closest_d) = polytope.closest_face();
        let normal = polytope.faces[closest_i].normal;

        let new_point = minkowski_support(a, b, normal);
        let projection = new_point.vertex.dot(&normal);

        // If the support function returns a vertex we already have, the
        // polytope cannot grow further in this direction, which means the
        // closest face is a genuine face of the Minkowski difference: that is
        // the exact answer, so stop.
        let duplicate = polytope
            .verts
            .iter()
            .any(|v| (v.vertex - new_point.vertex).length_squared() < 1e-12);

        if duplicate || projection - closest_d < EPA_TOLERANCE {
            let f = polytope.faces[closest_i];
            return Some(Contact {
                point_a: f.centroid_a(&polytope.verts),
                point_b: f.centroid_b(&polytope.verts),
                normal,
                depth: closest_d.max(0.0),
            });
        }

        if !polytope.expand(a, b, closest_i) {
            let f = polytope.faces[closest_i];
            return Some(Contact {
                point_a: f.centroid_a(&polytope.verts),
                point_b: f.centroid_b(&polytope.verts),
                normal: f.normal,
                depth: f.distance.max(0.0),
            });
        }
    }

    let (closest_i, closest_d) = polytope.closest_face();
    let f = polytope.faces[closest_i];
    Some(Contact {
        point_a: f.centroid_a(&polytope.verts),
        point_b: f.centroid_b(&polytope.verts),
        normal: f.normal,
        depth: closest_d.max(0.0),
    })
}

/// A resolved contact between two convex shapes, in world space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contact {
    /// Witness point on shape A.
    pub point_a: Vec3,
    /// Witness point on shape B.
    pub point_b: Vec3,
    /// Contact normal, pointing from A toward B.
    pub normal: Vec3,
    /// Penetration depth along the normal (>= 0).
    pub depth: f32,
}

impl Contact {
    /// The contact point in world space, midway between the two witnesses.
    pub fn point(&self) -> Vec3 {
        (self.point_a + self.point_b) * 0.5
    }

    /// Contact normal flipped so it points from B toward A.
    pub fn inverted_normal(&self) -> Vec3 {
        -self.normal
    }
}

/// Full narrowphase query: returns `None` when separated, otherwise the
/// contact information from EPA.
pub fn collide<A: SupportGeometry, B: SupportGeometry>(a: &A, b: &B) -> Option<Contact> {
    let (result, simplex) = gjk(a, b);
    match result {
        GjkResult::Intersecting => epa_penetration(a, b, &simplex),
        _ => None,
    }
}
