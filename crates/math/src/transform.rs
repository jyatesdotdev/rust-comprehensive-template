//! TRS transforms and camera matrices: [`Transform`], [`look_at_rh`],
//! [`perspective_rh`], [`orthographic_rh`].
//!
//! # Handedness and NDC — a crate-wide contract
//!
//! Everything in this module is **right-handed**: +X right, +Y up, and the
//! camera looks down **−Z** in view space. The projections target
//! **OpenGL-style normalized device coordinates**, where the visible volume
//! is `[-1, 1]` on *all three* axes (Vulkan and Direct3D use `[0, 1]` depth
//! instead — porting these matrices there requires a fix-up). The `_rh`
//! suffix on the functions records the convention, mirroring `glam`.
//!
//! The forthcoming `render` crate builds its entire camera and clipping
//! pipeline on exactly these choices. Do not change them silently.

use crate::mat::Mat4;
use crate::quat::Quat;
use crate::vec::Vec3;

/// A translation-rotation-scale transform: the standard decomposed form for
/// object placement (a "model" matrix), kept as three parts because they are
/// trivial to edit and interpolate individually — unlike a baked matrix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// Where the object sits in world space (applied last).
    pub translation: Vec3,
    /// The object's orientation (applied second).
    pub rotation: Quat,
    /// Per-axis scale factors (applied first).
    pub scale: Vec3,
}

impl Transform {
    /// The do-nothing transform: zero translation, identity rotation,
    /// unit scale.
    pub const IDENTITY: Self = Self {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    /// Create a transform from its three parts.
    pub fn new(translation: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Self {
            translation,
            rotation,
            scale,
        }
    }

    /// Bake into a single matrix, composed as `T · R · S`.
    ///
    /// Read right-to-left (matrices apply to the vector nearest them
    /// first): scale in the object's own frame, then rotate, then move into
    /// place. The other orders produce shearing (scale after rotate) or
    /// orbiting (translate before rotate) — almost never what you want.
    pub fn to_mat4(self) -> Mat4 {
        let r = self.rotation.to_mat3();
        // Column-major shortcut: R·S just scales column i of R by scale[i],
        // and T lands in the fourth column.
        Mat4 {
            cols: [
                [
                    r.cols[0][0] * self.scale.x,
                    r.cols[0][1] * self.scale.x,
                    r.cols[0][2] * self.scale.x,
                    0.0,
                ],
                [
                    r.cols[1][0] * self.scale.y,
                    r.cols[1][1] * self.scale.y,
                    r.cols[1][2] * self.scale.y,
                    0.0,
                ],
                [
                    r.cols[2][0] * self.scale.z,
                    r.cols[2][1] * self.scale.z,
                    r.cols[2][2] * self.scale.z,
                    0.0,
                ],
                [
                    self.translation.x,
                    self.translation.y,
                    self.translation.z,
                    1.0,
                ],
            ],
        }
    }
}

impl Default for Transform {
    /// The identity transform (note: unit scale, not zero scale).
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// Build a right-handed view matrix: world space → view space, with the
/// camera at `eye` looking toward `target`, and `up` fixing the roll.
///
/// In the resulting space the camera sits at the origin looking down **−Z**
/// (the crate convention), with +X right and +Y up.
///
/// Returns `None` when the frame is degenerate: `eye == target` (no view
/// direction) or `up` parallel to the view direction (no way to pick
/// "sideways"). Both fall out naturally as failed normalizations.
pub fn look_at_rh(eye: Vec3, target: Vec3, up: Vec3) -> Option<Mat4> {
    let f = (target - eye).normalize()?; // forward
    let s = f.cross(up).normalize()?; // sideways (right)
    let u = s.cross(f); // true up — unit because s ⊥ f are unit

    // Rows are the camera basis vectors (the inverse of a rotation is its
    // transpose), and the last column undoes the eye position.
    Some(Mat4 {
        cols: [
            [s.x, u.x, -f.x, 0.0],
            [s.y, u.y, -f.y, 0.0],
            [s.z, u.z, -f.z, 0.0],
            [-s.dot(eye), -u.dot(eye), f.dot(eye), 1.0],
        ],
    })
}

/// Build a right-handed perspective projection targeting OpenGL `[-1, 1]`
/// NDC on all axes.
///
/// `fov_y` is the *vertical* field of view in radians; `aspect` is
/// width/height; `near`/`far` are the **positive** distances to the clip
/// planes (the planes themselves sit at `z = -near` and `z = -far`, since
/// the camera looks down −Z).
///
/// The caller must ensure `0 < fov_y < π`, `aspect > 0`, and
/// `0 < near < far`; degenerate inputs produce non-finite matrix entries
/// (never a panic). Depth maps non-linearly: `z = -near` → NDC −1,
/// `z = -far` → NDC +1, with most precision near the near plane.
pub fn perspective_rh(fov_y: f64, aspect: f64, near: f64, far: f64) -> Mat4 {
    // Focal length: how far a unit of view-space height lands from the
    // screen center. Larger fov → shorter focal length.
    let f = 1.0 / (fov_y * 0.5).tan();
    Mat4 {
        cols: [
            [f / aspect, 0.0, 0.0, 0.0],
            [0.0, f, 0.0, 0.0],
            // Third column: maps z into [-1, 1] and copies -z into w (the
            // perspective divide is what makes far things small).
            [0.0, 0.0, (far + near) / (near - far), -1.0],
            [0.0, 0.0, (2.0 * far * near) / (near - far), 0.0],
        ],
    }
}

/// Build a right-handed orthographic projection targeting OpenGL `[-1, 1]`
/// NDC: the axis-aligned box `[left, right] × [bottom, top]` at view-space
/// depths `-near` to `-far` maps to the unit cube. No perspective divide —
/// parallel lines stay parallel (CAD views, shadow maps, 2D UI).
///
/// The caller must ensure `left != right`, `bottom != top`, `near != far`;
/// degenerate inputs produce non-finite matrix entries (never a panic).
pub fn orthographic_rh(left: f64, right: f64, bottom: f64, top: f64, near: f64, far: f64) -> Mat4 {
    // Each axis is an independent scale-and-shift onto [-1, 1].
    Mat4 {
        cols: [
            [2.0 / (right - left), 0.0, 0.0, 0.0],
            [0.0, 2.0 / (top - bottom), 0.0, 0.0],
            [0.0, 0.0, -2.0 / (far - near), 0.0],
            [
                -(right + left) / (right - left),
                -(top + bottom) / (top - bottom),
                -(far + near) / (far - near),
                1.0,
            ],
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec::approx_eq;
    use std::f64::consts::FRAC_PI_2;

    fn v3_approx_eq(a: Vec3, b: Vec3) -> bool {
        approx_eq(a.x, b.x) && approx_eq(a.y, b.y) && approx_eq(a.z, b.z)
    }

    #[test]
    fn identity_transform_is_identity_matrix() {
        assert_eq!(Transform::IDENTITY.to_mat4(), Mat4::IDENTITY);
        assert_eq!(Transform::default(), Transform::IDENTITY);
    }

    #[test]
    fn trs_order_scale_then_rotate_then_translate() {
        let t = Transform::new(
            Vec3::new(10.0, 0.0, 0.0),
            Quat::from_axis_angle(Vec3::Z, FRAC_PI_2),
            Vec3::new(2.0, 3.0, 1.0),
        );
        let m = t.to_mat4();
        let p = Vec3::new(1.0, 1.0, 0.0);
        // By hand: scale → (2, 3, 0); rotate 90° about Z → (-3, 2, 0);
        // translate → (7, 2, 0).
        assert!(v3_approx_eq(
            m.transform_point3(p),
            Vec3::new(7.0, 2.0, 0.0)
        ));
        // And it must equal applying the three parts one at a time.
        let manual =
            t.rotation
                .rotate_vec3(Vec3::new(p.x * t.scale.x, p.y * t.scale.y, p.z * t.scale.z))
                + t.translation;
        assert!(v3_approx_eq(m.transform_point3(p), manual));
    }

    #[test]
    fn look_at_puts_eye_at_origin_looking_down_neg_z() {
        let eye = Vec3::new(4.0, 3.0, 7.0);
        let target = Vec3::new(1.0, -1.0, 2.0);
        let view = look_at_rh(eye, target, Vec3::Y).unwrap();
        // The eye maps to the view-space origin.
        assert!(v3_approx_eq(view.transform_point3(eye), Vec3::ZERO));
        // The target lands straight ahead: on the -Z axis, at its true
        // distance.
        let d = (target - eye).length();
        let target_view = view.transform_point3(target);
        assert!(v3_approx_eq(target_view, Vec3::new(0.0, 0.0, -d)));
        // A view matrix is rigid (rotation + translation): volumes are
        // preserved.
        assert!(approx_eq(view.determinant(), 1.0));
    }

    #[test]
    fn look_at_keeps_up_upward() {
        let view = look_at_rh(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y).unwrap();
        // Looking down -Z from +Z with +Y up is (almost) the identity
        // rotation: world +Y must stay +Y in view space.
        let up_view = view.transform_point3(Vec3::Y) - view.transform_point3(Vec3::ZERO);
        assert!(v3_approx_eq(up_view, Vec3::Y));
    }

    #[test]
    fn look_at_degenerate_inputs_return_none() {
        let eye = Vec3::new(1.0, 2.0, 3.0);
        // No view direction.
        assert!(look_at_rh(eye, eye, Vec3::Y).is_none());
        // Up parallel to the view direction: cannot pick a sideways axis.
        assert!(look_at_rh(Vec3::ZERO, Vec3::Y * 5.0, Vec3::Y).is_none());
    }

    #[test]
    fn perspective_maps_frustum_into_ndc_bounds() {
        let proj = perspective_rh(FRAC_PI_2, 16.0 / 9.0, 0.1, 100.0);
        // A point clearly inside the frustum (between near and far, within
        // the field of view) must land in [-1, 1]³.
        let p = proj.project_point3(Vec3::new(1.0, -0.5, -5.0)).unwrap();
        for c in [p.x, p.y, p.z] {
            assert!((-1.0..=1.0).contains(&c), "NDC component out of range: {c}");
        }
        // Depth endpoints: near plane → -1, far plane → +1.
        let near_ndc = proj.project_point3(Vec3::new(0.0, 0.0, -0.1)).unwrap();
        let far_ndc = proj.project_point3(Vec3::new(0.0, 0.0, -100.0)).unwrap();
        assert!(approx_eq(near_ndc.z, -1.0));
        assert!(approx_eq(far_ndc.z, 1.0));
        // Perspective foreshortening: the same offset appears smaller when
        // farther away.
        let close = proj.project_point3(Vec3::new(1.0, 0.0, -1.0)).unwrap();
        let far = proj.project_point3(Vec3::new(1.0, 0.0, -50.0)).unwrap();
        assert!(close.x.abs() > far.x.abs());
    }

    #[test]
    fn perspective_point_outside_frustum_leaves_ndc() {
        let proj = perspective_rh(FRAC_PI_2, 1.0, 0.1, 100.0);
        // Way off to the side at close range: x blows past +1.
        let p = proj.project_point3(Vec3::new(10.0, 0.0, -1.0)).unwrap();
        assert!(p.x > 1.0);
    }

    #[test]
    fn orthographic_maps_box_corners_to_unit_cube() {
        let proj = orthographic_rh(-2.0, 2.0, -1.0, 1.0, 0.1, 10.0);
        // (right, top, far plane) → (1, 1, 1).
        let hi = proj.transform_point3(Vec3::new(2.0, 1.0, -10.0));
        assert!(v3_approx_eq(hi, Vec3::ONE));
        // (left, bottom, near plane) → (-1, -1, -1).
        let lo = proj.transform_point3(Vec3::new(-2.0, -1.0, -0.1));
        assert!(v3_approx_eq(lo, -Vec3::ONE));
        // The box center maps to the NDC origin.
        let mid = proj.transform_point3(Vec3::new(0.0, 0.0, -5.05));
        assert!(v3_approx_eq(mid, Vec3::ZERO));
        // No perspective: w stays 1, so parallel lines stay parallel.
        let a = proj.transform_point3(Vec3::new(1.0, 0.0, -1.0));
        let b = proj.transform_point3(Vec3::new(1.0, 0.0, -9.0));
        assert!(approx_eq(a.x, b.x));
    }

    #[test]
    fn full_camera_chain_centers_the_target() {
        // Integration: model → view → projection, the pipeline `render`
        // will run. The looked-at point must land at the center of the
        // screen (NDC x = y = 0) at a valid depth.
        let target = Vec3::new(2.0, 1.0, -3.0);
        let view = look_at_rh(Vec3::new(6.0, 4.0, 2.0), target, Vec3::Y).unwrap();
        let proj = perspective_rh(1.0, 1.5, 0.5, 50.0);
        let ndc = (proj * view).project_point3(target).unwrap();
        assert!(approx_eq(ndc.x, 0.0));
        assert!(approx_eq(ndc.y, 0.0));
        assert!((-1.0..=1.0).contains(&ndc.z));
    }
}
