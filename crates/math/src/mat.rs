//! Square matrices: [`Mat3`] and [`Mat4`].
//!
//! # Storage is COLUMN-MAJOR — a crate-wide contract
//!
//! `cols[c][r]` is the element at row `r`, column `c`. Equivalently:
//! `cols[0]` is the whole first *column*, not the first row. This matches
//! OpenGL/glam and is what lets a `Mat4`'s translation live in `cols[3]`.
//! Downstream code (the `render` crate, GPU uniform uploads) relies on this
//! layout — never flip it. The tell-tale of getting it wrong is transforms
//! that behave like their own transpose.
//!
//! Matrices multiply *column* vectors on the right (`M * v`), so in a chain
//! `A * B * v` the matrix nearest the vector applies first.

use std::ops::Mul;

use crate::vec::{Vec3, Vec4};
use crate::EPSILON;

/// A 3×3 matrix, stored column-major. Used for pure rotations/linear maps
/// (no translation) — see [`Mat4`] for affine and projective transforms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat3 {
    /// Columns of the matrix: `cols[c][r]` is the element at row `r`,
    /// column `c` (column-major — see the module docs).
    pub cols: [[f64; 3]; 3],
}

impl Mat3 {
    /// The identity matrix (ones on the diagonal).
    pub const IDENTITY: Self = Self {
        cols: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    };

    /// Build a matrix from its three columns (the images of the X, Y, and Z
    /// basis vectors).
    pub fn from_cols(x: Vec3, y: Vec3, z: Vec3) -> Self {
        Self {
            cols: [[x.x, x.y, x.z], [y.x, y.y, y.z], [z.x, z.y, z.z]],
        }
    }

    /// Return column `index` as a vector.
    ///
    /// # Panics
    ///
    /// Panics if `index >= 3` (plain array indexing).
    pub fn col(self, index: usize) -> Vec3 {
        let c = self.cols[index];
        Vec3::new(c[0], c[1], c[2])
    }

    /// Multiply this matrix by a column vector: `self * v`.
    pub fn mul_vec(self, v: Vec3) -> Vec3 {
        // Row r of the result dots row r of the matrix with v; with
        // column-major storage, row r is spread across the columns.
        Vec3::new(
            self.cols[0][0] * v.x + self.cols[1][0] * v.y + self.cols[2][0] * v.z,
            self.cols[0][1] * v.x + self.cols[1][1] * v.y + self.cols[2][1] * v.z,
            self.cols[0][2] * v.x + self.cols[1][2] * v.y + self.cols[2][2] * v.z,
        )
    }

    /// Transpose: rows become columns. For a pure rotation matrix the
    /// transpose is also the inverse.
    pub fn transpose(self) -> Self {
        let mut out = [[0.0; 3]; 3];
        for (c, col) in self.cols.iter().enumerate() {
            for (r, &val) in col.iter().enumerate() {
                out[r][c] = val;
            }
        }
        Self { cols: out }
    }

    /// Determinant of the 2×2 matrix left after deleting `row` and `col`.
    fn minor(self, row: usize, col: usize) -> f64 {
        // Collect the remaining 4 entries in column-major order.
        let mut m = [0.0; 4];
        let mut i = 0;
        for (c, column) in self.cols.iter().enumerate() {
            if c == col {
                continue;
            }
            for (r, &val) in column.iter().enumerate() {
                if r == row {
                    continue;
                }
                m[i] = val;
                i += 1;
            }
        }
        m[0] * m[3] - m[1] * m[2]
    }

    /// Determinant: the signed volume scale factor of the transform. Zero
    /// means the matrix collapses space onto a plane/line and has no inverse.
    pub fn determinant(self) -> f64 {
        // Laplace expansion along row 0: alternating signs times minors.
        (0..3)
            .map(|c| {
                let sign = if c % 2 == 0 { 1.0 } else { -1.0 };
                sign * self.cols[c][0] * self.minor(0, c)
            })
            .sum()
    }

    /// Inverse via the adjugate (transposed cofactor matrix) divided by the
    /// determinant. Returns `None` when `|det| <` [`EPSILON`] — the matrix is
    /// singular (or so close that the division would be numeric noise).
    pub fn inverse(self) -> Option<Self> {
        let det = self.determinant();
        if det.abs() < EPSILON {
            return None;
        }
        let inv_det = 1.0 / det;
        let mut out = [[0.0; 3]; 3];
        for (c, out_col) in out.iter_mut().enumerate() {
            for (r, entry) in out_col.iter_mut().enumerate() {
                // inverse(r, c) = cofactor(c, r) / det — note the swap
                // (that swap is the "adjugate transpose").
                let sign = if (r + c) % 2 == 0 { 1.0 } else { -1.0 };
                *entry = sign * self.minor(c, r) * inv_det;
            }
        }
        Some(Self { cols: out })
    }
}

impl Default for Mat3 {
    /// Identity, not the zero matrix — a zero default silently erases
    /// geometry the first time it multiplies something.
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Mul for Mat3 {
    type Output = Self;

    /// Matrix product. Column `j` of `A * B` is `A` applied to column `j` of
    /// `B` — with column-major storage the product is just three
    /// matrix-vector multiplies.
    fn mul(self, rhs: Self) -> Self {
        Self::from_cols(
            self.mul_vec(rhs.col(0)),
            self.mul_vec(rhs.col(1)),
            self.mul_vec(rhs.col(2)),
        )
    }
}

/// A 4×4 matrix, stored column-major. Represents affine transforms
/// (rotation/scale in the upper 3×3, translation in `cols[3]`) and
/// projections in homogeneous coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat4 {
    /// Columns of the matrix: `cols[c][r]` is the element at row `r`,
    /// column `c` (column-major — see the module docs).
    pub cols: [[f64; 4]; 4],
}

impl Mat4 {
    /// The identity matrix (ones on the diagonal).
    pub const IDENTITY: Self = Self {
        cols: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    /// Build a matrix from its four columns.
    pub fn from_cols(x: Vec4, y: Vec4, z: Vec4, w: Vec4) -> Self {
        Self {
            cols: [
                [x.x, x.y, x.z, x.w],
                [y.x, y.y, y.z, y.w],
                [z.x, z.y, z.z, z.w],
                [w.x, w.y, w.z, w.w],
            ],
        }
    }

    /// Return column `index` as a vector.
    ///
    /// # Panics
    ///
    /// Panics if `index >= 4` (plain array indexing).
    pub fn col(self, index: usize) -> Vec4 {
        let c = self.cols[index];
        Vec4::new(c[0], c[1], c[2], c[3])
    }

    /// Multiply this matrix by a homogeneous column vector: `self * v`.
    pub fn mul_vec(self, v: Vec4) -> Vec4 {
        Vec4::new(
            self.cols[0][0] * v.x
                + self.cols[1][0] * v.y
                + self.cols[2][0] * v.z
                + self.cols[3][0] * v.w,
            self.cols[0][1] * v.x
                + self.cols[1][1] * v.y
                + self.cols[2][1] * v.z
                + self.cols[3][1] * v.w,
            self.cols[0][2] * v.x
                + self.cols[1][2] * v.y
                + self.cols[2][2] * v.z
                + self.cols[3][2] * v.w,
            self.cols[0][3] * v.x
                + self.cols[1][3] * v.y
                + self.cols[2][3] * v.z
                + self.cols[3][3] * v.w,
        )
    }

    /// Transform a 3D point by an **affine** matrix (bottom row `0 0 0 1`):
    /// extends `p` with `w = 1`, so translation applies, and drops the
    /// unchanged `w`. For projection matrices use [`Self::project_point3`].
    pub fn transform_point3(self, p: Vec3) -> Vec3 {
        let v = self.mul_vec(Vec4::new(p.x, p.y, p.z, 1.0));
        Vec3::new(v.x, v.y, v.z)
    }

    /// Transform a point by a **projective** matrix and perform the
    /// perspective divide (`xyz / w`), yielding normalized device
    /// coordinates. Returns `None` when `|w| <` [`EPSILON`] — the point is
    /// on the camera plane and has no finite projection.
    pub fn project_point3(self, p: Vec3) -> Option<Vec3> {
        let v = self.mul_vec(Vec4::new(p.x, p.y, p.z, 1.0));
        if v.w.abs() < EPSILON {
            return None;
        }
        Some(Vec3::new(v.x / v.w, v.y / v.w, v.z / v.w))
    }

    /// Transpose: rows become columns.
    pub fn transpose(self) -> Self {
        let mut out = [[0.0; 4]; 4];
        for (c, col) in self.cols.iter().enumerate() {
            for (r, &val) in col.iter().enumerate() {
                out[r][c] = val;
            }
        }
        Self { cols: out }
    }

    /// Determinant of the 3×3 matrix left after deleting `row` and `col`.
    fn minor(self, row: usize, col: usize) -> f64 {
        // Collect the remaining 9 entries in column-major order, then apply
        // the 3×3 determinant formula (m[3c + r] is row r, column c).
        let mut m = [0.0; 9];
        let mut i = 0;
        for (c, column) in self.cols.iter().enumerate() {
            if c == col {
                continue;
            }
            for (r, &val) in column.iter().enumerate() {
                if r == row {
                    continue;
                }
                m[i] = val;
                i += 1;
            }
        }
        m[0] * (m[4] * m[8] - m[7] * m[5]) - m[3] * (m[1] * m[8] - m[7] * m[2])
            + m[6] * (m[1] * m[5] - m[4] * m[2])
    }

    /// Determinant: the signed volume scale factor of the transform. Zero
    /// means the matrix collapses space and has no inverse.
    pub fn determinant(self) -> f64 {
        // Laplace expansion along row 0.
        (0..4)
            .map(|c| {
                let sign = if c % 2 == 0 { 1.0 } else { -1.0 };
                sign * self.cols[c][0] * self.minor(0, c)
            })
            .sum()
    }

    /// Inverse via the adjugate (transposed cofactor matrix) divided by the
    /// determinant. Returns `None` when `|det| <` [`EPSILON`] — the matrix is
    /// singular (or so close that the division would be numeric noise).
    ///
    /// O(n⁵)-ish cofactor expansion is fine at 4×4 and easy to read;
    /// production libraries use specialized formulas instead.
    pub fn inverse(self) -> Option<Self> {
        let det = self.determinant();
        if det.abs() < EPSILON {
            return None;
        }
        let inv_det = 1.0 / det;
        let mut out = [[0.0; 4]; 4];
        for (c, out_col) in out.iter_mut().enumerate() {
            for (r, entry) in out_col.iter_mut().enumerate() {
                // inverse(r, c) = cofactor(c, r) / det — note the swap
                // (that swap is the "adjugate transpose").
                let sign = if (r + c) % 2 == 0 { 1.0 } else { -1.0 };
                *entry = sign * self.minor(c, r) * inv_det;
            }
        }
        Some(Self { cols: out })
    }
}

impl Default for Mat4 {
    /// Identity, not the zero matrix — a zero default silently erases
    /// geometry the first time it multiplies something.
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Mul for Mat4 {
    type Output = Self;

    /// Matrix product. Column `j` of `A * B` is `A` applied to column `j` of
    /// `B` — with column-major storage the product is just four
    /// matrix-vector multiplies.
    fn mul(self, rhs: Self) -> Self {
        Self::from_cols(
            self.mul_vec(rhs.col(0)),
            self.mul_vec(rhs.col(1)),
            self.mul_vec(rhs.col(2)),
            self.mul_vec(rhs.col(3)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec::approx_eq;

    fn mat3_approx_eq(a: Mat3, b: Mat3) -> bool {
        a.cols
            .iter()
            .flatten()
            .zip(b.cols.iter().flatten())
            .all(|(&x, &y)| approx_eq(x, y))
    }

    fn mat4_approx_eq(a: Mat4, b: Mat4) -> bool {
        a.cols
            .iter()
            .flatten()
            .zip(b.cols.iter().flatten())
            .all(|(&x, &y)| approx_eq(x, y))
    }

    /// A well-conditioned, non-symmetric 3×3 test matrix.
    fn sample_mat3() -> Mat3 {
        Mat3::from_cols(
            Vec3::new(2.0, 0.0, 1.0),
            Vec3::new(-1.0, 3.0, 0.5),
            Vec3::new(0.0, 1.0, 4.0),
        )
    }

    /// A well-conditioned, non-symmetric 4×4 test matrix.
    fn sample_mat4() -> Mat4 {
        Mat4::from_cols(
            Vec4::new(2.0, 0.0, 1.0, 0.0),
            Vec4::new(-1.0, 3.0, 0.5, 0.0),
            Vec4::new(0.0, 1.0, 4.0, -1.0),
            Vec4::new(5.0, -2.0, 0.0, 1.0),
        )
    }

    #[test]
    fn storage_is_column_major() {
        let m = Mat3::from_cols(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(4.0, 5.0, 6.0),
            Vec3::new(7.0, 8.0, 9.0),
        );
        // from_cols(x, ..) puts x into cols[0]; element (row 2, col 0) is 3.
        assert_eq!(m.cols[0], [1.0, 2.0, 3.0]);
        assert_eq!(m.cols[0][2], 3.0);
        assert_eq!(m.col(1), Vec3::new(4.0, 5.0, 6.0));
    }

    #[test]
    fn mat3_identity_is_neutral() {
        let m = sample_mat3();
        // Multiplying by an exact identity is exact — `==` is safe here.
        assert_eq!(Mat3::IDENTITY * m, m);
        assert_eq!(m * Mat3::IDENTITY, m);
        let v = Vec3::new(1.0, -2.0, 3.0);
        assert_eq!(Mat3::IDENTITY.mul_vec(v), v);
        assert_eq!(Mat3::default(), Mat3::IDENTITY);
    }

    #[test]
    fn mat3_mul_vec_by_hand() {
        let m = sample_mat3();
        // Column-major: result = col0*x + col1*y + col2*z.
        let v = Vec3::new(1.0, 2.0, 3.0);
        let expect = m.col(0) * 1.0 + m.col(1) * 2.0 + m.col(2) * 3.0;
        let got = m.mul_vec(v);
        assert!(approx_eq(got.x, expect.x));
        assert!(approx_eq(got.y, expect.y));
        assert!(approx_eq(got.z, expect.z));
    }

    #[test]
    fn mat3_transpose() {
        let m = sample_mat3();
        let t = m.transpose();
        for c in 0..3 {
            for r in 0..3 {
                assert_eq!(m.cols[c][r], t.cols[r][c]);
            }
        }
        assert_eq!(t.transpose(), m);
    }

    #[test]
    fn mat3_determinant_known_values() {
        assert!(approx_eq(Mat3::IDENTITY.determinant(), 1.0));
        // Diagonal matrix: determinant is the product of the diagonal.
        let d = Mat3::from_cols(Vec3::X * 2.0, Vec3::Y * 3.0, Vec3::Z * 4.0);
        assert!(approx_eq(d.determinant(), 24.0));
        // det(Aᵀ) == det(A).
        let m = sample_mat3();
        assert!(approx_eq(m.determinant(), m.transpose().determinant()));
    }

    #[test]
    fn mat3_inverse_round_trip() {
        let m = sample_mat3();
        let inv = m.inverse().unwrap();
        assert!(mat3_approx_eq(inv * m, Mat3::IDENTITY));
        assert!(mat3_approx_eq(m * inv, Mat3::IDENTITY));
    }

    #[test]
    fn mat3_singular_has_no_inverse() {
        // Third column is the sum of the first two → linearly dependent.
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        let m = Mat3::from_cols(a, b, a + b);
        assert!(approx_eq(m.determinant(), 0.0));
        assert!(m.inverse().is_none());
    }

    #[test]
    fn mat4_identity_is_neutral() {
        let m = sample_mat4();
        assert_eq!(Mat4::IDENTITY * m, m);
        assert_eq!(m * Mat4::IDENTITY, m);
        let v = Vec4::new(1.0, -2.0, 3.0, 1.0);
        assert_eq!(Mat4::IDENTITY.mul_vec(v), v);
        assert_eq!(Mat4::default(), Mat4::IDENTITY);
    }

    #[test]
    fn mat4_mul_matches_association_with_vector() {
        // (A * B) * v == A * (B * v): the defining property of the product.
        let a = sample_mat4();
        let b = Mat4::from_cols(
            Vec4::new(1.0, 0.5, 0.0, 0.0),
            Vec4::new(0.0, 2.0, 1.0, 0.0),
            Vec4::new(-1.0, 0.0, 1.0, 0.5),
            Vec4::new(3.0, 1.0, 0.0, 1.0),
        );
        let v = Vec4::new(0.7, -1.3, 2.2, 1.0);
        let lhs = (a * b).mul_vec(v);
        let rhs = a.mul_vec(b.mul_vec(v));
        assert!(approx_eq(lhs.x, rhs.x));
        assert!(approx_eq(lhs.y, rhs.y));
        assert!(approx_eq(lhs.z, rhs.z));
        assert!(approx_eq(lhs.w, rhs.w));
    }

    #[test]
    fn mat4_transpose() {
        let m = sample_mat4();
        let t = m.transpose();
        for c in 0..4 {
            for r in 0..4 {
                assert_eq!(m.cols[c][r], t.cols[r][c]);
            }
        }
        assert_eq!(t.transpose(), m);
    }

    #[test]
    fn mat4_determinant_known_values() {
        assert!(approx_eq(Mat4::IDENTITY.determinant(), 1.0));
        let d = Mat4::from_cols(
            Vec4::new(2.0, 0.0, 0.0, 0.0),
            Vec4::new(0.0, 3.0, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 4.0, 0.0),
            Vec4::new(0.0, 0.0, 0.0, 5.0),
        );
        assert!(approx_eq(d.determinant(), 120.0));
    }

    #[test]
    fn mat4_inverse_round_trip() {
        let m = sample_mat4();
        let inv = m.inverse().unwrap();
        assert!(mat4_approx_eq(inv * m, Mat4::IDENTITY));
        assert!(mat4_approx_eq(m * inv, Mat4::IDENTITY));
    }

    #[test]
    fn mat4_singular_has_no_inverse() {
        // Two identical columns → determinant is exactly zero.
        let c0 = Vec4::new(1.0, 2.0, 3.0, 4.0);
        let m = Mat4::from_cols(c0, c0, Vec4::new(0.0, 1.0, 0.0, 0.0), Vec4::ONE);
        assert!(approx_eq(m.determinant(), 0.0));
        assert!(m.inverse().is_none());
    }

    #[test]
    fn mat4_transform_point3_applies_translation() {
        // Pure translation by (10, 20, 30) lives in cols[3].
        let mut t = Mat4::IDENTITY;
        t.cols[3] = [10.0, 20.0, 30.0, 1.0];
        let p = t.transform_point3(Vec3::new(1.0, 2.0, 3.0));
        assert!(approx_eq(p.x, 11.0));
        assert!(approx_eq(p.y, 22.0));
        assert!(approx_eq(p.z, 33.0));
    }

    #[test]
    fn mat4_project_point3_divides_by_w() {
        // A matrix whose bottom row copies z into w: projecting (x, y, z)
        // should yield (x/z, y/z, z/z).
        let mut m = Mat4::IDENTITY;
        m.cols[2][3] = 1.0; // row 3, column 2 → w += z
        m.cols[3][3] = 0.0;
        let p = m.project_point3(Vec3::new(4.0, 6.0, 2.0)).unwrap();
        assert!(approx_eq(p.x, 2.0));
        assert!(approx_eq(p.y, 3.0));
        assert!(approx_eq(p.z, 1.0));
        // z = 0 → w = 0 → no finite projection.
        assert!(m.project_point3(Vec3::new(1.0, 1.0, 0.0)).is_none());
    }
}
