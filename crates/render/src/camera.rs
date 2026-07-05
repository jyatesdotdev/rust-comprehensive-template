//! The camera: the point-to-pixel pipeline and its inverse: [`Camera`].
//!
//! A camera is nothing but two matrices and a viewport rule:
//!
//! ```text
//! world point ──view──▶ view space ──projection──▶ NDC ──viewport──▶ pixel
//!   (x,y,z)    look_at    camera at    perspective  [-1,1]³  Y flip   (px,py)
//!              _rh        origin,      _rh + ÷w
//!                         looking −Z
//! ```
//!
//! Ray tracing runs the *same* pipeline **backwards**: pixel → NDC →
//! view-space direction → world-space ray. [`Camera::world_to_ndc`] +
//! [`Camera::ndc_to_screen`] are the forward direction (what a rasterizer
//! does to every vertex); [`Camera::primary_ray`] is the inverse (what a ray
//! tracer does to every pixel). They are one bijection run both ways, and
//! the round-trip tests below pin that: project a point, then cast a ray
//! through the resulting pixel, and you must pass back through the point.
//!
//! Production equivalent: the forward half is what `wgpu`'s vertex stage +
//! fixed-function viewport hardware do; you only supply the matrices
//! (usually built with `glam`). The inverse half is the first loop of every
//! CPU/GPU path tracer.

use std::f64::consts::PI;

use math::{Mat4, Vec3};

use crate::geometry::Ray;

/// A perspective pinhole camera: a viewpoint (`eye`, `target`, `up`) plus a
/// frustum (`fov_y`, `aspect`, `near`, `far`).
///
/// Fields are private because they are *derived state*: the view matrix and
/// the camera basis vectors are computed once in [`Camera::new`] and must
/// stay consistent with each other. All matrices come from `math`
/// ([`math::look_at_rh`], [`math::perspective_rh`]) — this crate never builds
/// projection matrices by hand, so it inherits math's conventions
/// (right-handed, camera looks down −Z, OpenGL `[-1, 1]` NDC) by
/// construction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Camera position in world space.
    eye: Vec3,
    /// Unit basis: world-space "right" of the camera (+X in view space).
    right: Vec3,
    /// Unit basis: world-space "up" of the camera (+Y in view space).
    up: Vec3,
    /// Unit basis: world-space viewing direction (−Z in view space).
    forward: Vec3,
    /// tan(fov_y / 2): half the height of the image plane at distance 1.
    tan_half_fov: f64,
    /// Width / height of the image.
    aspect: f64,
    /// World → view matrix (from [`math::look_at_rh`]).
    view: Mat4,
    /// View → clip matrix (from [`math::perspective_rh`]).
    proj: Mat4,
}

impl Camera {
    /// Build a camera, validating every input.
    ///
    /// Returns `None` when the frame or frustum is degenerate:
    /// `eye == target`, `up` parallel to the view direction (both via
    /// [`math::look_at_rh`]), `fov_y` outside `(0, π)`, `aspect <= 0`, or
    /// not `0 < near < far`. `math::perspective_rh` documents that it would
    /// produce non-finite matrices for those inputs; we refuse them up front
    /// instead.
    ///
    /// `aspect` should equal `width / height` of the framebuffer you render
    /// to, or the image will be stretched.
    pub fn new(
        eye: Vec3,
        target: Vec3,
        up: Vec3,
        fov_y: f64,
        aspect: f64,
        near: f64,
        far: f64,
    ) -> Option<Self> {
        if fov_y <= 0.0 || fov_y >= PI || aspect <= 0.0 {
            return None;
        }
        if near <= 0.0 || far <= near {
            return None;
        }
        let view = math::look_at_rh(eye, target, up)?;
        // The same basis look_at_rh builds internally, kept for primary_ray:
        // the *columns* of the inverse view rotation.
        let forward = (target - eye).normalize()?;
        let right = forward.cross(up).normalize()?;
        let true_up = right.cross(forward); // unit: right ⊥ forward, both unit
        Some(Self {
            eye,
            right,
            up: true_up,
            forward,
            tan_half_fov: (fov_y * 0.5).tan(),
            aspect,
            view,
            proj: math::perspective_rh(fov_y, aspect, near, far),
        })
    }

    /// Camera position in world space.
    pub fn eye(&self) -> Vec3 {
        self.eye
    }

    /// The world → view matrix.
    pub fn view(&self) -> Mat4 {
        self.view
    }

    /// The view → clip projection matrix.
    pub fn projection(&self) -> Mat4 {
        self.proj
    }

    /// Forward pipeline, steps 1–2: world point → normalized device
    /// coordinates. Visible points land in `[-1, 1]` on all three axes.
    ///
    /// Returns `None` for points **at or behind** the camera plane
    /// (view-space `z >= 0`): behind the camera, the perspective divide by
    /// `w = -z_view < 0` flips both signs, and the point would reappear
    /// mirrored inside the frustum — a classic rendering bug. A point
    /// exactly *on* the plane has `w ≈ 0` and no finite projection (that
    /// case is also what [`math::Mat4::project_point3`] guards).
    ///
    /// Points in front of the camera but outside the frustum still project —
    /// they just land outside `[-1, 1]`. Clipping is the caller's decision.
    pub fn world_to_ndc(&self, point: Vec3) -> Option<Vec3> {
        let v = self.view.transform_point3(point);
        if v.z >= -math::EPSILON {
            return None; // behind the camera (or on its plane): see docs
        }
        self.proj.project_point3(v)
    }

    /// Forward pipeline, step 3 (the viewport transform): NDC → integer
    /// pixel coordinates, clamped into the image.
    ///
    /// # The Y flip
    ///
    /// NDC has **+Y up** (math convention, inherited from OpenGL); raster
    /// images put **row 0 at the top** and count downward (file-format and
    /// framebuffer convention). So x maps `[-1, 1] → [0, width)` directly,
    /// while y maps `[-1, 1] → (height, 0]` — negated. Forgetting this flip
    /// renders every image upside down.
    ///
    /// The NDC z (depth) is ignored here; a rasterizer would keep it for the
    /// depth test.
    pub fn ndc_to_screen(ndc: Vec3, width: usize, height: usize) -> (usize, usize) {
        let fx = (ndc.x + 1.0) * 0.5 * width as f64;
        let fy = (1.0 - ndc.y) * 0.5 * height as f64; // the Y flip
        let max_x = width.saturating_sub(1) as f64;
        let max_y = height.saturating_sub(1) as f64;
        // Truncation after clamping: NDC exactly +1 lands on the last pixel
        // instead of one past it.
        (fx.clamp(0.0, max_x) as usize, fy.clamp(0.0, max_y) as usize)
    }

    /// The inverse pipeline: pixel → world-space ray through that pixel's
    /// **center** (hence the `+ 0.5` — pixel `(0, 0)` is the top-left
    /// *square*, and its center sits half a pixel in).
    ///
    /// Runs [`Camera::ndc_to_screen`] backwards (including the Y flip), then
    /// undoes the projection analytically: at distance 1 down the view axis
    /// the frustum is `tan(fov_y/2)` tall and `tan(fov_y/2) · aspect` wide,
    /// so NDC scales directly into a view-space direction `(x, y, -1)`,
    /// which the camera basis carries into world space. Same math as
    /// [`Camera::world_to_ndc`], run in the opposite direction.
    ///
    /// `width`/`height` are the target image size in pixels; `px`/`py` may
    /// lie outside it (the ray just leaves the frustum).
    pub fn primary_ray(&self, px: usize, py: usize, width: usize, height: usize) -> Ray {
        let w = width.max(1) as f64;
        let h = height.max(1) as f64;
        // Pixel center → NDC (inverse viewport transform, Y flipped back).
        let ndc_x = (px as f64 + 0.5) / w * 2.0 - 1.0;
        let ndc_y = 1.0 - (py as f64 + 0.5) / h * 2.0;
        // NDC → view-space direction at unit depth (inverse projection).
        let vx = ndc_x * self.tan_half_fov * self.aspect;
        let vy = ndc_y * self.tan_half_fov;
        // View → world (inverse view: the camera basis columns).
        let dir = self.right * vx + self.up * vy + self.forward;
        // `dir` always has a full unit of `forward` in it, so its length is
        // >= 1 and normalize cannot fail; the fallback is defensive only.
        let direction = dir.normalize().unwrap_or(self.forward);
        Ray {
            origin: self.eye,
            direction,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{approx_eq, v3_approx_eq};
    use std::f64::consts::FRAC_PI_3;

    /// The camera most tests share: on +Z, looking at the origin.
    fn test_camera() -> Camera {
        Camera::new(
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::ZERO,
            Vec3::Y,
            FRAC_PI_3,
            64.0 / 48.0,
            0.1,
            100.0,
        )
        .unwrap()
    }

    #[test]
    fn degenerate_inputs_return_none() {
        let eye = Vec3::new(0.0, 0.0, 5.0);
        // eye == target: no view direction.
        assert!(Camera::new(eye, eye, Vec3::Y, 1.0, 1.0, 0.1, 100.0).is_none());
        // up parallel to the view direction.
        assert!(Camera::new(Vec3::ZERO, Vec3::Y, Vec3::Y, 1.0, 1.0, 0.1, 100.0).is_none());
        // Bad frustum parameters.
        assert!(Camera::new(eye, Vec3::ZERO, Vec3::Y, 0.0, 1.0, 0.1, 100.0).is_none());
        assert!(Camera::new(eye, Vec3::ZERO, Vec3::Y, PI, 1.0, 0.1, 100.0).is_none());
        assert!(Camera::new(eye, Vec3::ZERO, Vec3::Y, 1.0, -1.0, 0.1, 100.0).is_none());
        assert!(Camera::new(eye, Vec3::ZERO, Vec3::Y, 1.0, 1.0, 0.0, 100.0).is_none());
        assert!(Camera::new(eye, Vec3::ZERO, Vec3::Y, 1.0, 1.0, 5.0, 5.0).is_none());
    }

    #[test]
    fn target_projects_to_image_center() {
        let cam = test_camera();
        // The looked-at point sits dead ahead: NDC x = y = 0.
        let ndc = cam.world_to_ndc(Vec3::ZERO).unwrap();
        assert!(approx_eq(ndc.x, 0.0));
        assert!(approx_eq(ndc.y, 0.0));
        assert!((-1.0..=1.0).contains(&ndc.z));
        // And the viewport transform puts NDC (0,0) at the image center.
        let (px, py) = Camera::ndc_to_screen(ndc, 64, 48);
        assert_eq!((px, py), (32, 24));
    }

    #[test]
    fn point_behind_camera_is_none() {
        let cam = test_camera();
        // The camera is at z = 5 looking toward -Z; z = 10 is behind it.
        assert!(cam.world_to_ndc(Vec3::new(0.0, 0.0, 10.0)).is_none());
        // A point exactly on the camera plane has no finite projection.
        assert!(cam.world_to_ndc(Vec3::new(1.0, 0.0, 5.0)).is_none());
        // Just in front still projects.
        assert!(cam.world_to_ndc(Vec3::new(0.0, 0.0, 4.9)).is_some());
    }

    #[test]
    fn ndc_to_screen_flips_y_and_clamps() {
        // NDC +Y (up) must land on row 0 (top of the raster image).
        let (_, top) = Camera::ndc_to_screen(Vec3::new(0.0, 1.0, 0.0), 64, 48);
        assert_eq!(top, 0);
        let (_, bottom) = Camera::ndc_to_screen(Vec3::new(0.0, -1.0, 0.0), 64, 48);
        assert_eq!(bottom, 47); // clamped onto the last row, not one past
                                // NDC -X lands in column 0; +X on the last column.
        let (left, _) = Camera::ndc_to_screen(Vec3::new(-1.0, 0.0, 0.0), 64, 48);
        let (right, _) = Camera::ndc_to_screen(Vec3::new(1.0, 0.0, 0.0), 64, 48);
        assert_eq!((left, right), (0, 63));
        // Out-of-frustum NDC clamps into the image instead of wrapping.
        let (x, y) = Camera::ndc_to_screen(Vec3::new(-7.0, 9.0, 0.0), 64, 48);
        assert_eq!((x, y), (0, 0));
    }

    #[test]
    fn primary_ray_starts_at_eye_and_is_unit_length() {
        let cam = test_camera();
        let ray = cam.primary_ray(0, 0, 64, 48);
        assert!(v3_approx_eq(ray.origin, cam.eye()));
        assert!(approx_eq(ray.direction.length(), 1.0));
        // Top-left pixel: the ray must lean left (-X) and up (+Y) while
        // travelling forward (-Z for this camera).
        assert!(ray.direction.x < 0.0);
        assert!(ray.direction.y > 0.0);
        assert!(ray.direction.z < 0.0);
    }

    #[test]
    fn project_then_raycast_round_trip() {
        // Forward pipeline: world point → NDC → pixel. Inverse pipeline:
        // that pixel → ray. The ray must pass within a pixel's footprint of
        // the original point — the two directions are the same bijection.
        let (width, height) = (64_usize, 48_usize);
        let cam = test_camera();
        let point = Vec3::new(0.3, -0.2, 1.0);

        let ndc = cam.world_to_ndc(point).unwrap();
        let (px, py) = Camera::ndc_to_screen(ndc, width, height);
        let ray = cam.primary_ray(px, py, width, height);

        // Distance from the point to the closest point on the ray.
        let to_point = point - ray.origin;
        let along = to_point.dot(ray.direction);
        let closest = ray.origin + ray.direction * along;
        let miss_distance = (point - closest).length();

        // One pixel's world-space footprint at the point's distance:
        // frustum height there is 2·tan(fov/2)·distance, split over `height`
        // rows. The pixel *center* can sit up to ~a pixel diagonal from the
        // projected point, so allow 1.5 pixels.
        let distance = along;
        let pixel_size = 2.0 * (FRAC_PI_3 * 0.5).tan() * distance / height as f64;
        assert!(
            miss_distance < 1.5 * pixel_size,
            "ray misses the point by {miss_distance}, pixel size {pixel_size}"
        );
    }

    #[test]
    fn raycast_then_project_returns_the_same_pixel() {
        // The other direction of the symmetry: a pixel's primary ray,
        // followed some distance, projects back onto that exact pixel.
        let (width, height) = (64_usize, 48_usize);
        let cam = test_camera();
        for &(px, py) in &[(0_usize, 0_usize), (32, 24), (63, 47), (10, 40)] {
            let ray = cam.primary_ray(px, py, width, height);
            let point = ray.at(7.0);
            let ndc = cam.world_to_ndc(point).unwrap();
            assert_eq!(Camera::ndc_to_screen(ndc, width, height), (px, py));
        }
    }

    #[test]
    fn accessors_expose_consistent_matrices() {
        let cam = test_camera();
        // view() maps the eye to the view-space origin…
        assert!(v3_approx_eq(
            cam.view().transform_point3(cam.eye()),
            Vec3::ZERO
        ));
        // …and projection ∘ view centers the target (math's own contract).
        let ndc = (cam.projection() * cam.view())
            .project_point3(Vec3::ZERO)
            .unwrap();
        assert!(approx_eq(ndc.x, 0.0) && approx_eq(ndc.y, 0.0));
    }
}
