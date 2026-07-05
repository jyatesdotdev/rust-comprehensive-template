//! Unit quaternions ([`Quat`]) for 3D rotation.
//!
//! Why quaternions instead of rotation matrices or Euler angles? They
//! interpolate cleanly ([`Quat::slerp`]), compose cheaply (16 multiplies vs
//! 27), have no gimbal lock, and renormalizing after drift is one division
//! (re-orthonormalizing a matrix is much messier).
//!
//! Everything here assumes **unit** quaternions (`length == 1`). The
//! constructors ([`Quat::IDENTITY`], [`Quat::from_axis_angle`]) uphold that
//! invariant, and products of unit quaternions stay unit up to float drift —
//! call [`Quat::normalize`] occasionally in long-running composition chains.

use std::ops::{Mul, Neg};

use crate::mat::{Mat3, Mat4};
use crate::vec::Vec3;
use crate::EPSILON;

/// Cosine threshold above which [`Quat::slerp`] switches to normalized
/// linear interpolation: nearly-parallel quaternions make the slerp
/// denominator `sin(θ) ≈ 0`, and nlerp is indistinguishable at such small
/// angles anyway.
const SLERP_DOT_THRESHOLD: f64 = 0.9995;

/// A rotation, stored as a unit quaternion `w + xi + yj + zk`.
///
/// `(x, y, z)` is the rotation axis scaled by `sin(angle/2)`; `w` is
/// `cos(angle/2)`. `q` and `-q` encode the *same* rotation (the double
/// cover) — [`Quat::slerp`] has to account for this.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quat {
    /// Vector part, X (axis.x · sin(angle/2)).
    pub x: f64,
    /// Vector part, Y (axis.y · sin(angle/2)).
    pub y: f64,
    /// Vector part, Z (axis.z · sin(angle/2)).
    pub z: f64,
    /// Scalar part (cos(angle/2)).
    pub w: f64,
}

impl Quat {
    /// The identity rotation (rotates nothing).
    pub const IDENTITY: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };

    /// Create a quaternion from raw components. Callers are responsible for
    /// the unit-length invariant — prefer [`Quat::from_axis_angle`], or
    /// follow up with [`Quat::normalize`].
    pub fn new(x: f64, y: f64, z: f64, w: f64) -> Self {
        Self { x, y, z, w }
    }

    /// Rotation of `angle` radians (counter-clockwise per the right-hand
    /// rule: thumb along `axis`, fingers curl in the rotation direction).
    ///
    /// `axis` need not be unit length — it is normalized here. If `axis` is
    /// near zero (below [`EPSILON`]) there is no direction to rotate around,
    /// so the identity rotation is returned.
    pub fn from_axis_angle(axis: Vec3, angle: f64) -> Self {
        match axis.normalize() {
            Some(n) => {
                let half = angle * 0.5;
                let s = half.sin();
                Self {
                    x: n.x * s,
                    y: n.y * s,
                    z: n.z * s,
                    w: half.cos(),
                }
            }
            None => Self::IDENTITY,
        }
    }

    /// Four-component dot product. For unit quaternions this is the cosine
    /// of half the angle between the two rotations; negative means they lie
    /// on opposite sides of the double cover.
    pub fn dot(self, rhs: Self) -> f64 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z + self.w * rhs.w
    }

    /// Squared length. Exactly 1 for a perfect unit quaternion.
    pub fn length_squared(self) -> f64 {
        self.dot(self)
    }

    /// Length (norm).
    pub fn length(self) -> f64 {
        self.length_squared().sqrt()
    }

    /// Rescale to unit length, restoring the invariant after float drift.
    /// Returns `None` if the length is below [`EPSILON`] (no direction to
    /// preserve).
    pub fn normalize(self) -> Option<Self> {
        let len = self.length();
        if len < EPSILON {
            None
        } else {
            Some(Self {
                x: self.x / len,
                y: self.y / len,
                z: self.z / len,
                w: self.w / len,
            })
        }
    }

    /// Conjugate: negates the vector part. For a **unit** quaternion the
    /// conjugate is the inverse — it undoes the rotation.
    pub fn conjugate(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
            z: -self.z,
            w: self.w,
        }
    }

    /// Rotate a vector by this quaternion.
    ///
    /// This is `q · (0, v) · q⁻¹` algebraically, but computed with the
    /// standard two-cross-product shortcut (Rodrigues-style), which is both
    /// faster and easier to follow than expanding two Hamilton products.
    pub fn rotate_vec3(self, v: Vec3) -> Vec3 {
        let u = Vec3::new(self.x, self.y, self.z);
        let t = u.cross(v) * 2.0;
        v + t * self.w + u.cross(t)
    }

    /// Spherical linear interpolation from `self` (`t = 0`) to `rhs`
    /// (`t = 1`) along the shortest arc, at constant angular velocity.
    ///
    /// Two edge cases are handled explicitly:
    ///
    /// - **Antipodal ambiguity**: `q` and `-q` are the same rotation, so if
    ///   `dot < 0` we negate `rhs` to take the short way around instead of
    ///   swinging up to 360° − θ the long way.
    /// - **Near-parallel inputs**: as θ → 0 the formula divides by
    ///   `sin(θ) → 0`, so above [`SLERP_DOT_THRESHOLD`] we fall back to
    ///   normalized linear interpolation, which is numerically safe and
    ///   visually identical at tiny angles.
    pub fn slerp(self, rhs: Self, t: f64) -> Self {
        let mut end = rhs;
        let mut dot = self.dot(rhs);
        if dot < 0.0 {
            end = -end;
            dot = -dot;
        }

        if dot > SLERP_DOT_THRESHOLD {
            // Nlerp fallback. The lerp of two nearly-equal unit quaternions
            // is nowhere near zero, so normalize() cannot fail here; the
            // unwrap_or keeps the library path panic-free regardless.
            let lerped = Self {
                x: self.x + (end.x - self.x) * t,
                y: self.y + (end.y - self.y) * t,
                z: self.z + (end.z - self.z) * t,
                w: self.w + (end.w - self.w) * t,
            };
            return lerped.normalize().unwrap_or(Self::IDENTITY);
        }

        // Standard slerp: sin-weighted combination that stays on the unit
        // sphere and moves at constant angular speed.
        let theta = dot.acos();
        let sin_theta = theta.sin();
        let a = ((1.0 - t) * theta).sin() / sin_theta;
        let b = (t * theta).sin() / sin_theta;
        Self {
            x: a * self.x + b * end.x,
            y: a * self.y + b * end.y,
            z: a * self.z + b * end.z,
            w: a * self.w + b * end.w,
        }
    }

    /// Convert to a 3×3 rotation matrix (column-major, like everything in
    /// this crate). Assumes `self` is unit length.
    pub fn to_mat3(self) -> Mat3 {
        // Precompute the doubled products; each matrix entry is then a
        // one-line combination (see any graphics text for the derivation).
        let (x2, y2, z2) = (self.x + self.x, self.y + self.y, self.z + self.z);
        let (xx, yy, zz) = (self.x * x2, self.y * y2, self.z * z2);
        let (xy, xz, yz) = (self.x * y2, self.x * z2, self.y * z2);
        let (wx, wy, wz) = (self.w * x2, self.w * y2, self.w * z2);
        Mat3 {
            cols: [
                [1.0 - yy - zz, xy + wz, xz - wy],
                [xy - wz, 1.0 - xx - zz, yz + wx],
                [xz + wy, yz - wx, 1.0 - xx - yy],
            ],
        }
    }

    /// Convert to a 4×4 homogeneous rotation matrix: the [`Self::to_mat3`]
    /// rotation in the upper-left, no translation.
    pub fn to_mat4(self) -> Mat4 {
        let m = self.to_mat3();
        Mat4 {
            cols: [
                [m.cols[0][0], m.cols[0][1], m.cols[0][2], 0.0],
                [m.cols[1][0], m.cols[1][1], m.cols[1][2], 0.0],
                [m.cols[2][0], m.cols[2][1], m.cols[2][2], 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }
}

impl Default for Quat {
    /// The identity rotation.
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Mul for Quat {
    type Output = Self;

    /// Hamilton product. `a * b` is the rotation "first apply `b`, then
    /// apply `a`" — same right-to-left convention as matrix multiplication.
    /// Not commutative.
    fn mul(self, rhs: Self) -> Self {
        Self {
            x: self.w * rhs.x + self.x * rhs.w + self.y * rhs.z - self.z * rhs.y,
            y: self.w * rhs.y - self.x * rhs.z + self.y * rhs.w + self.z * rhs.x,
            z: self.w * rhs.z + self.x * rhs.y - self.y * rhs.x + self.z * rhs.w,
            w: self.w * rhs.w - self.x * rhs.x - self.y * rhs.y - self.z * rhs.z,
        }
    }
}

impl Neg for Quat {
    type Output = Self;

    /// Negate all four components. The result represents the *same rotation*
    /// (double cover) but sits on the opposite side of the 4D unit sphere —
    /// which matters for interpolation.
    fn neg(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
            z: -self.z,
            w: -self.w,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec::approx_eq;
    use std::f64::consts::{FRAC_PI_2, FRAC_PI_4};

    fn v3_approx_eq(a: Vec3, b: Vec3) -> bool {
        approx_eq(a.x, b.x) && approx_eq(a.y, b.y) && approx_eq(a.z, b.z)
    }

    fn quat_approx_eq(a: Quat, b: Quat) -> bool {
        approx_eq(a.x, b.x) && approx_eq(a.y, b.y) && approx_eq(a.z, b.z) && approx_eq(a.w, b.w)
    }

    /// Same *rotation*, allowing for the q / -q double cover.
    fn same_rotation(a: Quat, b: Quat) -> bool {
        quat_approx_eq(a, b) || quat_approx_eq(a, -b)
    }

    #[test]
    fn identity_rotates_nothing() {
        let v = Vec3::new(1.0, -2.0, 3.0);
        assert!(v3_approx_eq(Quat::IDENTITY.rotate_vec3(v), v));
        assert_eq!(Quat::default(), Quat::IDENTITY);
    }

    #[test]
    fn from_axis_angle_quarter_turn_about_z() {
        // 90° about +Z takes +X to +Y (right-hand rule).
        let q = Quat::from_axis_angle(Vec3::Z, FRAC_PI_2);
        assert!(v3_approx_eq(q.rotate_vec3(Vec3::X), Vec3::Y));
        assert!(v3_approx_eq(q.rotate_vec3(Vec3::Y), -Vec3::X));
        assert!(approx_eq(q.length(), 1.0));
    }

    #[test]
    fn from_axis_angle_normalizes_axis_and_handles_zero() {
        // A scaled axis gives the same rotation as the unit axis.
        let a = Quat::from_axis_angle(Vec3::Y * 42.0, 0.7);
        let b = Quat::from_axis_angle(Vec3::Y, 0.7);
        assert!(quat_approx_eq(a, b));
        // A zero axis has no direction: identity.
        assert_eq!(Quat::from_axis_angle(Vec3::ZERO, 1.0), Quat::IDENTITY);
    }

    #[test]
    fn hamilton_product_composes_rotations() {
        // 90° about Z then 90° about X, composed right-to-left.
        let rz = Quat::from_axis_angle(Vec3::Z, FRAC_PI_2);
        let rx = Quat::from_axis_angle(Vec3::X, FRAC_PI_2);
        let combined = rx * rz;
        let v = Vec3::new(1.0, 0.0, 0.0);
        let step_by_step = rx.rotate_vec3(rz.rotate_vec3(v));
        assert!(v3_approx_eq(combined.rotate_vec3(v), step_by_step));
        // X → (rz) → Y → (rx) → Z.
        assert!(v3_approx_eq(combined.rotate_vec3(Vec3::X), Vec3::Z));
    }

    #[test]
    fn conjugate_undoes_rotation() {
        let q = Quat::from_axis_angle(Vec3::new(1.0, 2.0, 3.0), 0.9);
        let v = Vec3::new(-0.3, 1.7, 2.2);
        assert!(v3_approx_eq(q.conjugate().rotate_vec3(q.rotate_vec3(v)), v));
        // q * q⁻¹ = identity.
        assert!(same_rotation(q * q.conjugate(), Quat::IDENTITY));
    }

    #[test]
    fn normalize_restores_unit_length() {
        let q = Quat::new(2.0, 0.0, 0.0, 2.0).normalize().unwrap();
        assert!(approx_eq(q.length(), 1.0));
        assert!(Quat::new(0.0, 0.0, 0.0, 0.0).normalize().is_none());
    }

    #[test]
    fn rotation_agrees_with_matrix_form() {
        // The whole point of to_mat3: it must be the same linear map.
        let q = Quat::from_axis_angle(Vec3::new(1.0, -2.0, 0.5), 1.3);
        let m = q.to_mat3();
        for v in [Vec3::X, Vec3::Y, Vec3::Z, Vec3::new(0.4, -1.1, 2.7)] {
            assert!(v3_approx_eq(m.mul_vec(v), q.rotate_vec3(v)));
        }
        // And the matrix of a rotation is orthonormal: det = +1.
        assert!(approx_eq(m.determinant(), 1.0));
    }

    #[test]
    fn to_mat4_embeds_rotation_with_no_translation() {
        let q = Quat::from_axis_angle(Vec3::Y, FRAC_PI_4);
        let m4 = q.to_mat4();
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert!(v3_approx_eq(m4.transform_point3(v), q.rotate_vec3(v)));
        assert_eq!(m4.cols[3], [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn slerp_endpoints() {
        let a = Quat::from_axis_angle(Vec3::X, 0.3);
        let b = Quat::from_axis_angle(Vec3::Y, 1.1);
        assert!(same_rotation(a.slerp(b, 0.0), a));
        assert!(same_rotation(a.slerp(b, 1.0), b));
    }

    #[test]
    fn slerp_midpoint_is_half_angle() {
        // Halfway between identity and a 90° turn is a 45° turn.
        let a = Quat::IDENTITY;
        let b = Quat::from_axis_angle(Vec3::Z, FRAC_PI_2);
        let mid = a.slerp(b, 0.5);
        let expected = Quat::from_axis_angle(Vec3::Z, FRAC_PI_4);
        assert!(same_rotation(mid, expected));
        // Slerp output stays on the unit sphere.
        assert!(approx_eq(mid.length(), 1.0));
    }

    #[test]
    fn slerp_takes_short_arc_for_antipodal_inputs() {
        // -b encodes the same rotation as b; slerp must not take the long
        // way around the 4D sphere when handed the negated form.
        let a = Quat::from_axis_angle(Vec3::Z, 0.2);
        let b = Quat::from_axis_angle(Vec3::Z, 1.2);
        let via_b = a.slerp(b, 0.5);
        let via_neg_b = a.slerp(-b, 0.5);
        assert!(same_rotation(via_b, via_neg_b));
        assert!(same_rotation(via_b, Quat::from_axis_angle(Vec3::Z, 0.7)));
    }

    #[test]
    fn slerp_near_parallel_falls_back_to_nlerp() {
        // Angle so small that dot > threshold: exercises the nlerp branch.
        let a = Quat::from_axis_angle(Vec3::Y, 0.0);
        let b = Quat::from_axis_angle(Vec3::Y, 1e-5);
        let mid = a.slerp(b, 0.5);
        assert!(approx_eq(mid.length(), 1.0));
        assert!(same_rotation(mid, Quat::from_axis_angle(Vec3::Y, 5e-6)));
        // Identical inputs interpolate to themselves.
        assert!(same_rotation(a.slerp(a, 0.5), a));
    }
}
