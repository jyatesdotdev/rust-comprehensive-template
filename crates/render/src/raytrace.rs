//! A minimal but real ray tracer: [`Framebuffer`], [`Scene`], [`render`].
//!
//! This module is the proof that the other three compose: for every pixel,
//! [`crate::Camera`] inverts the viewing pipeline into a ray,
//! [`crate::Sphere`] finds the nearest surface, and Lambertian `n·l`
//! shading produces a [`crate::LinearRgb`] that stays linear until
//! [`Framebuffer::to_ppm`] quantizes it to sRGB. No recursion, no shadows,
//! no sampling — one primary ray per pixel, so every step stays
//! inspectable.
//!
//! Production equivalent: the per-pixel loop is exactly what a `wgpu`
//! fragment shader (rasterizing) or compute shader (ray tracing) runs in
//! parallel on the GPU; the `image` crate would replace the toy PPM writer.

use math::Vec3;

use crate::camera::Camera;
use crate::color::LinearRgb;
use crate::geometry::{Hit, Ray, Sphere};

/// A grid of linear-light pixels, row-major with **row 0 at the top** (the
/// raster convention — see [`Camera::ndc_to_screen`] for the Y flip that
/// puts it there).
#[derive(Debug, Clone, PartialEq)]
pub struct Framebuffer {
    /// Number of pixel columns.
    pub width: usize,
    /// Number of pixel rows.
    pub height: usize,
    /// Row-major pixel storage: pixel `(x, y)` lives at `y * width + x`.
    /// Length is always `width * height`.
    pub pixels: Vec<LinearRgb>,
}

impl Framebuffer {
    /// Create a framebuffer of `width × height` pixels, all black.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![LinearRgb::BLACK; width * height],
        }
    }

    /// The pixel at `(x, y)`, or `None` when the coordinates fall outside
    /// the image (no panicking indexing in library paths).
    pub fn pixel(&self, x: usize, y: usize) -> Option<LinearRgb> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.pixels.get(y * self.width + x).copied()
    }

    /// Set the pixel at `(x, y)`; out-of-range coordinates are ignored.
    pub fn set_pixel(&mut self, x: usize, y: usize, color: LinearRgb) {
        if x >= self.width || y >= self.height {
            return;
        }
        if let Some(p) = self.pixels.get_mut(y * self.width + x) {
            *p = color;
        }
    }

    /// Encode as a plain-text PPM (`P3`) image — the simplest file format a
    /// viewer understands, writable with nothing but `std`.
    ///
    /// Layout: a header (`P3`, dimensions, the maximum channel value 255),
    /// then one `r g b` triple per line in row order, top row first. This is
    /// the single place linear light becomes sRGB (via
    /// [`LinearRgb::to_srgb_u8`]) — everything upstream stays linear.
    pub fn to_ppm(&self) -> String {
        // Header + roughly 12 bytes per pixel line.
        let mut out = String::with_capacity(32 + self.pixels.len() * 12);
        out.push_str(&format!("P3\n{} {}\n255\n", self.width, self.height));
        for pixel in &self.pixels {
            let [r, g, b] = pixel.to_srgb_u8();
            out.push_str(&format!("{r} {g} {b}\n"));
        }
        out
    }
}

/// The world to render: colored spheres, one directional light, and a
/// background color for rays that hit nothing.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    /// The objects: each sphere paired with its albedo (the fraction of
    /// each channel it reflects, in linear space).
    pub spheres: Vec<(Sphere, LinearRgb)>,
    /// Direction **from any surface toward the light** (a "sun" infinitely
    /// far away, so it is the same everywhere). It is normalized once per
    /// [`render`] call; a zero vector means "no light" and every surface
    /// shades black.
    pub light_dir: Vec3,
    /// Color returned for rays that miss everything.
    pub background: LinearRgb,
}

/// Render `scene` through `camera` into every pixel of `framebuffer`.
///
/// For each pixel: cast the primary ray ([`Camera::primary_ray`]), find the
/// nearest sphere hit (smallest `t` — painter's logic done right), and
/// shade it as a Lambertian (perfectly matte) surface:
///
/// ```text
/// color = albedo · max(n · l, 0)
/// ```
///
/// `n·l` is the cosine of the angle between the surface normal and the
/// light: full brightness facing the light, falling to zero edge-on. The
/// clamp matters — for surfaces facing *away*, the dot product goes
/// negative, and without the clamp they would subtract light and go darker
/// than black. `camera`'s aspect ratio should match the framebuffer's, or
/// the image stretches.
pub fn render(scene: &Scene, camera: &Camera, framebuffer: &mut Framebuffer) {
    let (width, height) = (framebuffer.width, framebuffer.height);
    let light = scene.light_dir.normalize(); // None → unlit scene
    for (i, pixel) in framebuffer.pixels.iter_mut().enumerate() {
        // Recover (x, y) from the row-major index.
        let (px, py) = (i % width.max(1), i / width.max(1));
        let ray = camera.primary_ray(px, py, width, height);
        *pixel = shade(scene, ray, light);
    }
}

/// Color seen along one ray: nearest hit shaded Lambertian, else background.
fn shade(scene: &Scene, ray: Ray, light: Option<Vec3>) -> LinearRgb {
    let mut nearest: Option<(Hit, LinearRgb)> = None;
    for &(sphere, albedo) in &scene.spheres {
        if let Some(hit) = sphere.intersect(ray) {
            let is_closer = nearest.map_or(true, |(best, _)| hit.t < best.t);
            if is_closer {
                nearest = Some((hit, albedo));
            }
        }
    }
    match (nearest, light) {
        // Lambertian: albedo scaled by the clamped cosine term.
        (Some((hit, albedo)), Some(l)) => albedo * hit.normal.dot(l).max(0.0),
        // A scene without a light renders its objects black…
        (Some(_), None) => LinearRgb::BLACK,
        // …and rays that miss everything see the background either way.
        (None, _) => scene.background,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::approx_eq;
    use std::f64::consts::FRAC_PI_3;

    const WIDTH: usize = 32;
    const HEIGHT: usize = 24;

    /// Camera at +Z looking at the origin, matched to the 32×24 buffer.
    fn test_camera() -> Camera {
        Camera::new(
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::ZERO,
            Vec3::Y,
            FRAC_PI_3,
            WIDTH as f64 / HEIGHT as f64,
            0.1,
            100.0,
        )
        .unwrap()
    }

    /// One red unit sphere at the origin, lit from the camera's direction.
    fn test_scene() -> Scene {
        Scene {
            spheres: vec![(Sphere::new(Vec3::ZERO, 1.0), LinearRgb::new(0.8, 0.1, 0.1))],
            light_dir: Vec3::Z, // toward the light, i.e. behind the camera
            background: LinearRgb::new(0.0, 0.0, 0.25),
        }
    }

    #[test]
    fn framebuffer_get_set_and_bounds() {
        let mut fb = Framebuffer::new(4, 3);
        assert_eq!(fb.pixels.len(), 12);
        assert_eq!(fb.pixel(0, 0), Some(LinearRgb::BLACK));
        fb.set_pixel(3, 2, LinearRgb::WHITE);
        assert_eq!(fb.pixel(3, 2), Some(LinearRgb::WHITE));
        // Out of range: read is None, write is a no-op (not a panic).
        assert_eq!(fb.pixel(4, 0), None);
        assert_eq!(fb.pixel(0, 3), None);
        fb.set_pixel(4, 0, LinearRgb::WHITE);
        fb.set_pixel(0, 3, LinearRgb::WHITE);
        assert_eq!(
            fb.pixels.iter().filter(|&&p| p == LinearRgb::WHITE).count(),
            1
        );
    }

    #[test]
    fn end_to_end_render_lights_the_sphere() {
        let scene = test_scene();
        let camera = test_camera();
        let mut fb = Framebuffer::new(WIDTH, HEIGHT);
        render(&scene, &camera, &mut fb);

        // Center pixel: the ray hits the sphere nearly head-on, the normal
        // faces the light (both ≈ +Z), so n·l ≈ 1 and the pixel is close to
        // the full albedo.
        let center = fb.pixel(WIDTH / 2, HEIGHT / 2).unwrap();
        let albedo = scene.spheres[0].1;
        assert!(
            (center.r - albedo.r).abs() < 0.05,
            "center.r = {}",
            center.r
        );
        assert!((center.g - albedo.g).abs() < 0.05);
        assert!((center.b - albedo.b).abs() < 0.05);

        // All four corner rays miss the sphere: exact background color.
        for &(x, y) in &[
            (0, 0),
            (WIDTH - 1, 0),
            (0, HEIGHT - 1),
            (WIDTH - 1, HEIGHT - 1),
        ] {
            assert_eq!(fb.pixel(x, y), Some(scene.background));
        }

        // The limb of the sphere faces sideways (n·l → 0): darker than the
        // center but still a sphere pixel. Grab one partway out.
        let limb = fb.pixel(WIDTH / 2, HEIGHT / 2 - 4).unwrap();
        assert!(limb.r < center.r);
    }

    #[test]
    fn render_without_light_shades_objects_black() {
        let mut scene = test_scene();
        scene.light_dir = Vec3::ZERO; // no direction → no light
        let camera = test_camera();
        let mut fb = Framebuffer::new(WIDTH, HEIGHT);
        render(&scene, &camera, &mut fb);
        assert_eq!(fb.pixel(WIDTH / 2, HEIGHT / 2), Some(LinearRgb::BLACK));
        assert_eq!(fb.pixel(0, 0), Some(scene.background));
    }

    #[test]
    fn nearest_sphere_wins() {
        // Two spheres on the view axis: the closer one must own the pixel.
        let scene = Scene {
            spheres: vec![
                (
                    Sphere::new(Vec3::new(0.0, 0.0, -3.0), 1.0),
                    LinearRgb::WHITE,
                ),
                (Sphere::new(Vec3::ZERO, 1.0), LinearRgb::new(1.0, 0.0, 0.0)),
            ],
            light_dir: Vec3::Z,
            background: LinearRgb::BLACK,
        };
        let camera = test_camera();
        let mut fb = Framebuffer::new(WIDTH, HEIGHT);
        render(&scene, &camera, &mut fb);
        let center = fb.pixel(WIDTH / 2, HEIGHT / 2).unwrap();
        // The red sphere (closer to the camera at +Z) hides the white one.
        assert!(approx_eq(center.g, 0.0) && approx_eq(center.b, 0.0));
        assert!(center.r > 0.9);
    }

    #[test]
    fn ppm_header_and_pixel_count() {
        let scene = test_scene();
        let camera = test_camera();
        let mut fb = Framebuffer::new(WIDTH, HEIGHT);
        render(&scene, &camera, &mut fb);

        let ppm = fb.to_ppm();
        let mut lines = ppm.lines();
        assert_eq!(lines.next(), Some("P3"));
        assert_eq!(lines.next(), Some("32 24"));
        assert_eq!(lines.next(), Some("255"));
        // One "r g b" line per pixel, each channel a valid 0..=255 value.
        let pixel_lines: Vec<&str> = lines.collect();
        assert_eq!(pixel_lines.len(), WIDTH * HEIGHT);
        for line in &pixel_lines {
            let channels: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(channels.len(), 3);
            for c in channels {
                assert!(c.parse::<u8>().is_ok(), "bad channel value: {c}");
            }
        }
        // First pixel line is the top-left corner: the background color.
        let bg = scene.background.to_srgb_u8();
        assert_eq!(pixel_lines[0], format!("{} {} {}", bg[0], bg[1], bg[2]));
    }
}
