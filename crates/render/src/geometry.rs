//! Rays and analytic ray–surface intersection: [`Ray`], [`Hit`], [`Sphere`],
//! [`Aabb`], [`Plane`].
//!
//! Every intersection routine answers the same question: *for the smallest
//! `t > 0`, where does `origin + direction · t` first touch the surface?*
//! Returning the **nearest positive** `t` is what makes a ray tracer show the
//! front-most object; a "hit" at `t <= 0` would be behind the camera.
//!
//! All routines treat `t <= EPSILON` as "no hit" rather than exactly
//! `t <= 0.0`: a hit at distance `1e-14` is indistinguishable from rounding
//! noise (e.g. a ray re-intersecting the surface it just left).
//!
//! Production equivalent: on a GPU this work is done either by hardware ray
//! tracing units (wgpu exposes them via `wgpu::Features::EXPERIMENTAL_RAY_QUERY`)
//! or not at all — the classic raster pipeline never intersects rays, it
//! projects triangles instead (see [`crate::camera`] for that direction).

use math::{Vec3, EPSILON};

/// Component `i` of a vector (0 → x, 1 → y, anything else → z).
///
/// [`Vec3`] has named fields, not an index operator; the AABB slab loop wants
/// to treat the three axes uniformly, so this tiny adapter lives here.
fn axis(v: Vec3, i: usize) -> f64 {
    match i {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

/// Unit vector along axis `i` (0 → +X, 1 → +Y, anything else → +Z).
fn axis_unit(i: usize) -> Vec3 {
    match i {
        0 => Vec3::X,
        1 => Vec3::Y,
        _ => Vec3::Z,
    }
}

/// A half-line: all points `origin + direction * t` for `t >= 0`.
///
/// # Invariant: `direction` is unit length
///
/// [`Ray::new`] enforces this by normalizing (and returns `None` for a
/// near-zero direction, which has no direction to normalize). The fields stay
/// public for ergonomics — like [`math::Quat`], the invariant is upheld by the
/// constructor and *assumed* everywhere else. Formulas that silently break on
/// a non-unit direction:
///
/// - [`Sphere::intersect`] drops the quadratic's `a = d·d` term because it
///   equals 1; with `|d| != 1` every reported `t` is wrong by `1/|d|²`-ish.
/// - `t` is only *distance along the ray* when `|d| = 1`; comparing hit
///   distances between objects, or against a light distance, breaks otherwise.
/// - Lambertian shading (`n · l`) assumes both vectors are unit length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    /// Where the ray starts.
    pub origin: Vec3,
    /// Unit-length travel direction (see the type-level invariant).
    pub direction: Vec3,
}

impl Ray {
    /// Create a ray, normalizing `direction`.
    ///
    /// Returns `None` when `direction` is shorter than [`math::EPSILON`] —
    /// a near-zero vector points nowhere, so no ray can be built from it.
    pub fn new(origin: Vec3, direction: Vec3) -> Option<Self> {
        Some(Self {
            origin,
            direction: direction.normalize()?,
        })
    }

    /// The point at parameter `t`: `origin + direction * t`.
    ///
    /// Because `direction` is unit length, `t` is also the *distance* from
    /// the origin to that point.
    pub fn at(self, t: f64) -> Vec3 {
        self.origin + self.direction * t
    }
}

/// The result of a successful ray–surface intersection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    /// Distance along the ray (equals world-space distance because ray
    /// directions are unit length). Always `>` [`math::EPSILON`].
    pub t: f64,
    /// The intersection point, `ray.at(t)`.
    pub point: Vec3,
    /// The surface's **geometric outward** unit normal at `point`. It is
    /// *not* flipped to face the ray: a ray leaving a sphere from inside, or
    /// exiting an AABB, sees a normal pointing *along* its travel direction.
    /// Callers that need a ray-facing normal must flip it themselves.
    pub normal: Vec3,
}

/// A sphere defined by center and radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sphere {
    /// Center of the sphere.
    pub center: Vec3,
    /// Radius. Must be positive: the normal is computed as
    /// `(point - center) / radius`, which a zero radius turns into
    /// non-finite garbage (no panic, just meaningless normals).
    pub radius: f64,
}

impl Sphere {
    /// Create a sphere. `radius` must be positive (see the field docs).
    pub fn new(center: Vec3, radius: f64) -> Self {
        Self { center, radius }
    }

    /// Nearest intersection with `ray` at `t >` [`math::EPSILON`], if any.
    ///
    /// Substituting the ray equation into `|p - center|² = r²` gives a
    /// quadratic in `t`: `t² + 2(oc·d)t + (|oc|² - r²) = 0` (the `t²`
    /// coefficient is 1 because ray directions are unit — see [`Ray`]).
    ///
    /// # Why the "stable" quadratic form
    ///
    /// The textbook roots `-h ± √(h² - c)` subtract two nearly equal numbers
    /// whenever the ray origin is far away or grazes the sphere (`h² ≫ c`,
    /// so `√(h²-c) ≈ |h|`). That *catastrophic cancellation* wipes out most
    /// significant digits of the smaller root. Instead we compute the root
    /// where the two terms **add** (`q = -(h + sign(h)·√disc)`, always safe)
    /// and recover the other via Vieta's formula `t₀·t₁ = c` — a division,
    /// which loses no precision.
    pub fn intersect(&self, ray: Ray) -> Option<Hit> {
        let oc = ray.origin - self.center;
        let half_b = oc.dot(ray.direction); // h in the docs above
        let c = oc.length_squared() - self.radius * self.radius;
        let disc = half_b * half_b - c;
        if disc < 0.0 {
            return None; // ray line misses the sphere entirely
        }
        let sqrt_disc = disc.sqrt();
        // The cancellation-free root: both terms have the same sign.
        let q = if half_b >= 0.0 {
            -(half_b + sqrt_disc)
        } else {
            sqrt_disc - half_b
        };
        let t0 = q;
        // Vieta: t0 * t1 = c. Guard q ≈ 0 (ray origin on the sphere, grazing)
        // so the division cannot produce inf/NaN.
        let t1 = if q.abs() < EPSILON { q } else { c / q };
        let (near, far) = if t0 < t1 { (t0, t1) } else { (t1, t0) };
        // Nearest root in front of the ray; the near root is negative when
        // the origin is inside the sphere (or the sphere is behind us).
        let t = if near > EPSILON {
            near
        } else if far > EPSILON {
            far
        } else {
            return None;
        };
        let point = ray.at(t);
        Some(Hit {
            t,
            point,
            // Dividing by the radius normalizes exactly (|point - center| = r).
            normal: (point - self.center) / self.radius,
        })
    }
}

/// An axis-aligned bounding box, the corner-to-corner volume `[min, max]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    /// Corner with the smallest x, y, z. Must be `<= max` componentwise.
    pub min: Vec3,
    /// Corner with the largest x, y, z.
    pub max: Vec3,
}

impl Aabb {
    /// Create a box from its two extreme corners (`min <= max` componentwise).
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// Nearest intersection with `ray` at `t >` [`math::EPSILON`], if any.
    ///
    /// # The slab method
    ///
    /// The box is the intersection of three *slabs* (the space between two
    /// parallel planes, one pair per axis). For each axis, the ray is inside
    /// that slab for a `t` interval; the ray is inside the **box** where all
    /// three intervals overlap. We shrink a running `[t_near, t_far]` window
    /// axis by axis and miss the moment it goes empty.
    ///
    /// # Why axis-parallel rays need the explicit guard
    ///
    /// When a direction component is 0, the popular trick is to divide anyway
    /// and let IEEE infinities do the work (`1/0 = ±inf` gives an infinite or
    /// empty interval). That *almost* works — but when the origin sits exactly
    /// on a slab plane, `(min - origin) * inf` becomes `0 · ∞ = NaN`, and one
    /// NaN poisons every subsequent min/max comparison differently depending
    /// on operand order. Rather than lean on those semantics (and on how
    /// `f64::min`/`max` happen to treat NaN), we guard explicitly: a ray
    /// parallel to a slab either misses outright (origin outside the slab) or
    /// imposes no constraint at all (origin inside it).
    pub fn intersect(&self, ray: Ray) -> Option<Hit> {
        let mut t_near = f64::NEG_INFINITY;
        let mut t_far = f64::INFINITY;
        // Which axis produced the current t_near / t_far — that axis's slab
        // plane is the face the ray enters / exits through.
        let mut near_axis = 0;
        let mut far_axis = 0;

        for i in 0..3 {
            let o = axis(ray.origin, i);
            let d = axis(ray.direction, i);
            let (lo, hi) = (axis(self.min, i), axis(self.max, i));

            if d.abs() < EPSILON {
                // Parallel to this slab: no crossing ever happens on this
                // axis, so it is all-or-nothing (see doc comment).
                if o < lo || o > hi {
                    return None;
                }
                continue;
            }

            let inv = 1.0 / d;
            let (t0, t1) = {
                let a = (lo - o) * inv;
                let b = (hi - o) * inv;
                if a <= b {
                    (a, b)
                } else {
                    (b, a)
                }
            };
            if t0 > t_near {
                t_near = t0;
                near_axis = i;
            }
            if t1 < t_far {
                t_far = t1;
                far_axis = i;
            }
            if t_near > t_far {
                return None; // the intervals no longer overlap: miss
            }
        }

        // Entering hit if the entry point is in front of the ray; otherwise
        // the origin is inside the box and the first surface we can touch is
        // the exit face.
        let (t, face_axis, entering) = if t_near > EPSILON {
            (t_near, near_axis, true)
        } else if t_far > EPSILON {
            (t_far, far_axis, false)
        } else {
            return None; // box entirely behind the origin
        };

        let d = axis(ray.direction, face_axis);
        // Geometric outward normal of the face that was hit. Entering through
        // a face means travelling *against* its outward normal; exiting means
        // travelling *with* it.
        let sign = if entering { -d.signum() } else { d.signum() };
        Some(Hit {
            t,
            point: ray.at(t),
            normal: axis_unit(face_axis) * sign,
        })
    }
}

/// An infinite plane: all points `p` with `normal · p + d = 0`.
///
/// `d` is the negated signed distance from the origin to the plane along
/// `normal` (e.g. the ground plane `y = -2` is `normal = +Y, d = 2`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    /// Unit normal of the plane (enforced by [`Plane::new`], same
    /// constructor-upheld pattern as [`Ray`]).
    pub normal: Vec3,
    /// Plane offset: `normal · p + d = 0` for points on the plane.
    pub d: f64,
}

impl Plane {
    /// Create a plane, normalizing `normal`.
    ///
    /// Returns `None` when `normal` is shorter than [`math::EPSILON`] —
    /// a plane needs an orientation.
    pub fn new(normal: Vec3, d: f64) -> Option<Self> {
        Some(Self {
            normal: normal.normalize()?,
            d,
        })
    }

    /// Nearest intersection with `ray` at `t >` [`math::EPSILON`], if any.
    ///
    /// Substituting the ray into the plane equation and solving for `t`
    /// gives `t = -(n·o + d) / (n·dir)`. A denominator near zero means the
    /// ray runs parallel to the plane: it never crosses (or lies inside the
    /// plane, which we also report as a miss — there is no single hit point).
    pub fn intersect(&self, ray: Ray) -> Option<Hit> {
        let denom = self.normal.dot(ray.direction);
        if denom.abs() < EPSILON {
            return None; // parallel (or contained in the plane)
        }
        let t = -(self.normal.dot(ray.origin) + self.d) / denom;
        if t <= EPSILON {
            return None; // the crossing is behind the ray origin
        }
        Some(Hit {
            t,
            point: ray.at(t),
            normal: self.normal,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{approx_eq, v3_approx_eq};

    #[test]
    fn ray_new_normalizes_direction() {
        let r = Ray::new(Vec3::ZERO, Vec3::new(0.0, 0.0, -5.0)).unwrap();
        assert!(v3_approx_eq(r.direction, -Vec3::Z));
        assert!(approx_eq(r.direction.length(), 1.0));
        // A zero direction has no direction: constructor refuses.
        assert!(Ray::new(Vec3::ZERO, Vec3::ZERO).is_none());
    }

    #[test]
    fn ray_at_is_distance_along_direction() {
        let r = Ray::new(Vec3::new(1.0, 2.0, 3.0), Vec3::X).unwrap();
        assert!(v3_approx_eq(r.at(4.0), Vec3::new(5.0, 2.0, 3.0)));
    }

    #[test]
    fn sphere_head_on_hit() {
        let s = Sphere::new(Vec3::ZERO, 1.0);
        let r = Ray::new(Vec3::new(0.0, 0.0, 5.0), -Vec3::Z).unwrap();
        let hit = s.intersect(r).unwrap();
        assert!(approx_eq(hit.t, 4.0)); // nearest of the two roots (4 and 6)
        assert!(v3_approx_eq(hit.point, Vec3::Z));
        assert!(v3_approx_eq(hit.normal, Vec3::Z)); // outward, toward the ray
    }

    #[test]
    fn sphere_miss() {
        let s = Sphere::new(Vec3::ZERO, 1.0);
        let r = Ray::new(Vec3::new(0.0, 2.0, 5.0), -Vec3::Z).unwrap();
        assert!(s.intersect(r).is_none());
    }

    #[test]
    fn sphere_behind_origin_is_a_miss() {
        let s = Sphere::new(Vec3::ZERO, 1.0);
        // Pointing away from the sphere: both roots are negative.
        let r = Ray::new(Vec3::new(0.0, 0.0, 5.0), Vec3::Z).unwrap();
        assert!(s.intersect(r).is_none());
    }

    #[test]
    fn sphere_tangent_hit() {
        let s = Sphere::new(Vec3::ZERO, 1.0);
        // Grazes the top of the unit sphere: discriminant is exactly zero,
        // both roots coincide at t = 5.
        let r = Ray::new(Vec3::new(-5.0, 1.0, 0.0), Vec3::X).unwrap();
        let hit = s.intersect(r).unwrap();
        assert!(approx_eq(hit.t, 5.0));
        assert!(v3_approx_eq(hit.point, Vec3::Y));
        assert!(v3_approx_eq(hit.normal, Vec3::Y));
    }

    #[test]
    fn sphere_from_inside_hits_far_wall_with_outward_normal() {
        let s = Sphere::new(Vec3::ZERO, 2.0);
        let r = Ray::new(Vec3::ZERO, Vec3::X).unwrap();
        let hit = s.intersect(r).unwrap();
        assert!(approx_eq(hit.t, 2.0)); // near root (-2) is behind us
        assert!(v3_approx_eq(hit.point, Vec3::X * 2.0));
        // Geometric outward normal points *along* the ray when exiting.
        assert!(v3_approx_eq(hit.normal, Vec3::X));
    }

    #[test]
    fn sphere_grazing_from_far_away_stays_accurate() {
        // Long, nearly tangential approach — the case the stable quadratic
        // form exists for. The hit point must still be on the sphere.
        let s = Sphere::new(Vec3::ZERO, 1.0);
        let r = Ray::new(Vec3::new(-1.0e8, 0.99, 0.0), Vec3::X).unwrap();
        let hit = s.intersect(r).unwrap();
        assert!((hit.point - s.center).length() - 1.0 < 1.0e-6);
        assert!(hit.t > 0.0);
    }

    #[test]
    fn aabb_hit_from_outside() {
        let b = Aabb::new(-Vec3::ONE, Vec3::ONE);
        let r = Ray::new(Vec3::new(5.0, 0.5, 0.5), -Vec3::X).unwrap();
        let hit = b.intersect(r).unwrap();
        assert!(approx_eq(hit.t, 4.0)); // enters through the +X face
        assert!(v3_approx_eq(hit.point, Vec3::new(1.0, 0.5, 0.5)));
        assert!(v3_approx_eq(hit.normal, Vec3::X)); // outward, opposing travel
    }

    #[test]
    fn aabb_origin_inside_hits_exit_face() {
        let b = Aabb::new(-Vec3::ONE, Vec3::ONE);
        let r = Ray::new(Vec3::ZERO, Vec3::new(0.0, -1.0, 0.0)).unwrap();
        let hit = b.intersect(r).unwrap();
        assert!(approx_eq(hit.t, 1.0));
        assert!(v3_approx_eq(hit.point, -Vec3::Y));
        // Outward normal of the -Y face points along the ray (we exit it).
        assert!(v3_approx_eq(hit.normal, -Vec3::Y));
    }

    #[test]
    fn aabb_axis_parallel_ray_inside_slabs_hits() {
        let b = Aabb::new(-Vec3::ONE, Vec3::ONE);
        // Direction has zero y and z: parallel to two slab pairs, but the
        // origin lies inside both, so only the x slabs constrain the ray.
        let r = Ray::new(Vec3::new(-5.0, 0.25, -0.25), Vec3::X).unwrap();
        let hit = b.intersect(r).unwrap();
        assert!(approx_eq(hit.t, 4.0));
        assert!(v3_approx_eq(hit.normal, -Vec3::X));
    }

    #[test]
    fn aabb_axis_parallel_ray_outside_slab_misses() {
        let b = Aabb::new(-Vec3::ONE, Vec3::ONE);
        // Parallel to the y slabs but starting above them: can never enter.
        let r = Ray::new(Vec3::new(-5.0, 2.0, 0.0), Vec3::X).unwrap();
        assert!(b.intersect(r).is_none());
    }

    #[test]
    fn aabb_axis_parallel_ray_origin_on_slab_boundary() {
        let b = Aabb::new(-Vec3::ONE, Vec3::ONE);
        // Origin exactly on the y = 1 plane with zero y direction — the
        // 0·∞ = NaN case the explicit guard exists for. On the boundary
        // counts as inside the slab, so the ray hits the box edge-on.
        let r = Ray::new(Vec3::new(-5.0, 1.0, 0.0), Vec3::X).unwrap();
        let hit = b.intersect(r).unwrap();
        assert!(approx_eq(hit.t, 4.0));
    }

    #[test]
    fn aabb_diagonal_miss_and_behind_miss() {
        let b = Aabb::new(-Vec3::ONE, Vec3::ONE);
        // Slab intervals exist per axis but never overlap.
        let miss = Ray::new(Vec3::new(5.0, 5.0, 0.0), Vec3::new(-1.0, 1.0, 0.0)).unwrap();
        assert!(b.intersect(miss).is_none());
        // Box entirely behind the ray.
        let behind = Ray::new(Vec3::new(5.0, 0.0, 0.0), Vec3::X).unwrap();
        assert!(b.intersect(behind).is_none());
    }

    #[test]
    fn plane_hit() {
        // Ground plane y = -2: normal +Y, d = 2.
        let p = Plane::new(Vec3::Y, 2.0).unwrap();
        let r = Ray::new(Vec3::new(0.0, 1.0, 0.0), -Vec3::Y).unwrap();
        let hit = p.intersect(r).unwrap();
        assert!(approx_eq(hit.t, 3.0));
        assert!(v3_approx_eq(hit.point, Vec3::new(0.0, -2.0, 0.0)));
        assert!(v3_approx_eq(hit.normal, Vec3::Y));
    }

    #[test]
    fn plane_parallel_ray_misses() {
        let p = Plane::new(Vec3::Y, 0.0).unwrap();
        // Travelling parallel to the plane, above it.
        let r = Ray::new(Vec3::new(0.0, 1.0, 0.0), Vec3::X).unwrap();
        assert!(p.intersect(r).is_none());
        // Even a ray *inside* the plane reports no single hit point.
        let inside = Ray::new(Vec3::ZERO, Vec3::X).unwrap();
        assert!(p.intersect(inside).is_none());
    }

    #[test]
    fn plane_crossing_behind_origin_is_a_miss() {
        let p = Plane::new(Vec3::Y, 2.0).unwrap();
        // Moving away from the plane: the crossing is at negative t.
        let r = Ray::new(Vec3::new(0.0, 1.0, 0.0), Vec3::Y).unwrap();
        assert!(p.intersect(r).is_none());
    }

    #[test]
    fn plane_new_normalizes_and_rejects_zero_normal() {
        let p = Plane::new(Vec3::Y * 10.0, 1.0).unwrap();
        assert!(approx_eq(p.normal.length(), 1.0));
        assert!(Plane::new(Vec3::ZERO, 1.0).is_none());
    }
}
