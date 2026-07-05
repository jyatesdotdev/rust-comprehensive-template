//! Hand-rolled linear algebra for 3D graphics, in pure `std`.
//!
//! This is a **foundation crate** (like `common`): the forthcoming `render`
//! crate builds directly on it. It exists to teach the math that production
//! libraries hide behind SIMD and macros — every dot product, inverse, and
//! projection here is written out longhand so it can be read and checked
//! against a textbook.
//!
//! # Conventions — load-bearing contracts for dependents
//!
//! Downstream crates (`render`) assume these hold everywhere. Changing any of
//! them is a silent, workspace-wide breaking change:
//!
//! - **All scalars are `f64`.** No `f32` anywhere.
//! - **Matrices are column-major** ([`Mat3`], [`Mat4`]): `cols[c][r]` is the
//!   element at row `r`, column `c`, and a matrix transforms a *column*
//!   vector on its right (`M * v`).
//! - **Right-handed coordinates**: +X right, +Y up, the camera looks down
//!   **−Z** (`x_axis.cross(y_axis) == z_axis`).
//! - **Projections target OpenGL-style NDC**: visible points map into
//!   `[-1, 1]` on all three axes (not Vulkan/D3D `[0, 1]` depth).
//!
//! # Production equivalents
//!
//! Use a real library outside this template: [`glam`](https://docs.rs/glam)
//! is the standard for games/rendering (f32, SIMD-accelerated), and
//! [`nalgebra`](https://docs.rs/nalgebra) covers general-purpose and
//! dimension-generic linear algebra. Their APIs mirror what you see here.
//!
//! ```rust
//! use math::{Mat4, Quat, Transform, Vec3};
//!
//! let model = Transform::new(
//!     Vec3::new(0.0, 1.0, 0.0),
//!     Quat::from_axis_angle(Vec3::Y, 0.5),
//!     Vec3::ONE,
//! )
//! .to_mat4();
//! let view = math::look_at_rh(Vec3::new(0.0, 2.0, 5.0), Vec3::ZERO, Vec3::Y)
//!     .expect("eye, target, and up are non-degenerate");
//! let proj = math::perspective_rh(std::f64::consts::FRAC_PI_3, 16.0 / 9.0, 0.1, 100.0);
//! let mvp: Mat4 = proj * view * model;
//! ```

pub mod mat;
pub mod quat;
pub mod transform;
pub mod vec;

// Re-export the main types so dependents can write `use math::{Vec3, Mat4}`.
pub use mat::{Mat3, Mat4};
pub use quat::Quat;
pub use transform::{look_at_rh, orthographic_rh, perspective_rh, Transform};
pub use vec::{Vec2, Vec3, Vec4};

/// Threshold below which a length or determinant is treated as zero.
///
/// Dividing by a value smaller than this amplifies rounding noise into
/// garbage, so [`Vec3::normalize`] (and friends) and the matrix `inverse`
/// methods return `None` instead. The value is absolute, not relative — good
/// enough for the unit-scale geometry this template works with, but a real
/// library would scale the tolerance to the operands.
pub const EPSILON: f64 = 1e-12;
