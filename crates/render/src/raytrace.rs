//! A minimal but real ray tracer: [`Framebuffer`], [`Material`], [`Scene`],
//! [`render`].
//!
//! This module is the proof that the other three compose: for every pixel,
//! [`crate::Camera`] inverts the viewing pipeline into a ray,
//! [`crate::Sphere`] finds the nearest surface, and Lambertian `n·l`
//! shading produces a [`crate::LinearRgb`] that stays linear until
//! [`Framebuffer::to_ppm`] quantizes it to sRGB. On top of that base the
//! tracer adds the two effects that need *secondary* rays:
//!
//! - **Shadows**: a ray from the hit point toward the light; any occluder
//!   kills the diffuse term, leaving only the ambient floor.
//! - **One-bounce reflection**: the primary ray mirrored about the normal
//!   (`r = d − 2(d·n)n`) and traced recursively, blended in by the
//!   material's reflectivity, with an explicit depth limit ([`Scene::max_depth`]).
//!
//! No sampling and no other global illumination — one primary ray per
//! pixel, so every step stays inspectable.
//!
//! Production equivalent: the per-pixel loop is exactly what a `wgpu`
//! fragment shader (rasterizing) or compute shader (ray tracing) runs in
//! parallel on the GPU; the `image` crate would replace the toy PPM writer.

use math::Vec3;

use crate::camera::Camera;
use crate::color::LinearRgb;
use crate::geometry::{Hit, Ray, Sphere};

/// Offset applied along the surface normal to the origin of every
/// *secondary* ray (shadow and reflection rays).
///
/// # Why: "shadow acne"
///
/// A hit point is computed as `ray.at(t)` in floating point, so it lands a
/// rounding error *near* the surface — sometimes a hair inside it. A shadow
/// ray started exactly there can re-intersect the very surface it left, and
/// the surface then reports itself as its own occluder. Because the rounding
/// error flips sign pixel to pixel, the result is a salt-and-pepper speckle
/// of false shadow — the classic **shadow acne** artifact. Nudging the
/// origin along the outward normal moves it decisively to the outside of
/// the surface before the secondary ray is cast.
///
/// # Why this value (a documented ad-hoc threshold)
///
/// [`math::EPSILON`] (`1e-12`) is a *relative*-scale "treat as zero" test;
/// the rounding error of `ray.at(t)` is *absolute* and grows with scene
/// scale (roughly `t · 1e-16`, so ~`1e-13` at distance 1000). `1e-6` clears
/// that error by many orders of magnitude while staying far below any
/// feature size a scene could visibly resolve.
const SHADOW_BIAS: f64 = 1e-6;

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

/// How a surface responds to light: a diffuse color plus how mirror-like it
/// is.
///
/// # Invariant: `reflectivity` lies in `[0, 1]`
///
/// Enforced by [`Material::new`] (which clamps) and assumed by the blend in
/// the tracer — like [`Ray`]'s unit direction, the fields stay public for
/// ergonomics and the constructor upholds the invariant. A value outside
/// `[0, 1]` would make `local·(1−k) + reflected·k` extrapolate, creating
/// light from nowhere.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material {
    /// Fraction of each channel the surface reflects diffusely, in linear
    /// space (the Lambertian albedo).
    pub albedo: LinearRgb,
    /// Mirror-likeness: `0.0` is perfectly matte (pure Lambertian), `1.0` a
    /// perfect mirror. Blends the local shade with the traced reflection.
    pub reflectivity: f64,
}

impl Material {
    /// Create a material, clamping `reflectivity` into `[0, 1]` (see the
    /// type-level invariant).
    pub fn new(albedo: LinearRgb, reflectivity: f64) -> Self {
        Self {
            albedo,
            reflectivity: reflectivity.clamp(0.0, 1.0),
        }
    }

    /// A perfectly matte (reflectivity 0) material — plain Lambertian, the
    /// exact behavior this tracer had before reflection existed.
    pub fn matte(albedo: LinearRgb) -> Self {
        Self {
            albedo,
            reflectivity: 0.0,
        }
    }
}

/// The world to render: spheres with materials, one directional light, a
/// background color, an ambient floor, and the reflection depth limit.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    /// The objects: each sphere paired with its [`Material`].
    pub spheres: Vec<(Sphere, Material)>,
    /// Direction **from any surface toward the light** (a "sun" infinitely
    /// far away, so it is the same everywhere). It is normalized once per
    /// [`render`] call; a zero vector means "no light" and every surface
    /// keeps only its ambient term.
    pub light_dir: Vec3,
    /// Color returned for rays that miss everything.
    pub background: LinearRgb,
    /// Ambient floor: the fraction of albedo every surface shows even in
    /// full shadow, standing in for light bounced off the rest of the world.
    /// Keep it small (`0.0`–`0.1`); large values wash out shading entirely.
    pub ambient: f64,
    /// Maximum number of reflection bounces per primary ray.
    ///
    /// # Why a depth limit must exist
    ///
    /// Two facing mirrors reflect each other forever: with `reflectivity`
    /// near 1 the recursion never runs out of light to follow, so without a
    /// hard cap the tracer recurses until the stack dies. Every real tracer
    /// bounds cost this way (possibly plus Russian roulette); each extra
    /// bounce also contributes geometrically less to the final pixel, so a
    /// small cap ([`Scene::DEFAULT_MAX_DEPTH`]) loses almost nothing.
    pub max_depth: u32,
}

impl Scene {
    /// Default reflection bounce cap: one mirror seen in another mirror.
    /// Deliberately small — see [`Scene::max_depth`] for the rationale.
    pub const DEFAULT_MAX_DEPTH: u32 = 2;
}

/// Render `scene` through `camera` into every pixel of `framebuffer`.
///
/// For each pixel: cast the primary ray ([`Camera::primary_ray`]), find the
/// nearest sphere hit (smallest `t` — painter's logic done right), and
/// shade it:
///
/// ```text
/// local = albedo · (ambient + shadow · max(n · l, 0))
/// color = local · (1 − reflectivity) + traced_reflection · reflectivity
/// ```
///
/// `n·l` is the cosine of the angle between the surface normal and the
/// light: full brightness facing the light, falling to zero edge-on. The
/// clamp matters — for surfaces facing *away*, the dot product goes
/// negative, and without the clamp they would subtract light and go darker
/// than black. `shadow` is 0 when another sphere blocks the path to the
/// light (only the ambient floor survives), 1 otherwise. Reflection traces
/// the mirrored ray recursively, at most [`Scene::max_depth`] bounces deep.
/// `camera`'s aspect ratio should match the framebuffer's, or the image
/// stretches.
pub fn render(scene: &Scene, camera: &Camera, framebuffer: &mut Framebuffer) {
    let (width, height) = (framebuffer.width, framebuffer.height);
    let light = scene.light_dir.normalize(); // None → unlit scene
    for (i, pixel) in framebuffer.pixels.iter_mut().enumerate() {
        // Recover (x, y) from the row-major index.
        let (px, py) = (i % width.max(1), i / width.max(1));
        let ray = camera.primary_ray(px, py, width, height);
        *pixel = trace(scene, ray, light, scene.max_depth);
    }
}

/// The nearest sphere hit along `ray`, with its material.
fn nearest_hit(scene: &Scene, ray: Ray) -> Option<(Hit, Material)> {
    let mut nearest: Option<(Hit, Material)> = None;
    for &(sphere, material) in &scene.spheres {
        if let Some(hit) = sphere.intersect(ray) {
            let is_closer = nearest.map_or(true, |(best, _)| hit.t < best.t);
            if is_closer {
                nearest = Some((hit, material));
            }
        }
    }
    nearest
}

/// Does any sphere block the path from `point` (with outward `normal`)
/// toward the light along unit direction `light`?
///
/// The shadow ray's origin is nudged along the normal by [`SHADOW_BIAS`] so
/// the surface cannot re-intersect itself (shadow acne — see the constant's
/// docs). The light is directional (infinitely far away), so *any* hit at
/// positive `t` occludes; there is no light distance to compare against.
fn in_shadow(scene: &Scene, point: Vec3, normal: Vec3, light: Vec3) -> bool {
    // `light` is unit length (normalized once in `render`), so building the
    // Ray literal directly upholds the unit-direction invariant.
    let shadow_ray = Ray {
        origin: point + normal * SHADOW_BIAS,
        direction: light,
    };
    scene
        .spheres
        .iter()
        .any(|&(sphere, _)| sphere.intersect(shadow_ray).is_some())
}

/// Color seen along one ray, following reflections at most `depth` bounces.
///
/// The `reflectivity == 0` fast path returns the local Lambertian shade
/// untouched (no blend arithmetic), so matte scenes reproduce the plain
/// Lambertian tracer bit for bit — a documented editing rule in AGENTS.md.
fn trace(scene: &Scene, ray: Ray, light: Option<Vec3>, depth: u32) -> LinearRgb {
    let Some((hit, material)) = nearest_hit(scene, ray) else {
        return scene.background; // missed everything: the sky
    };

    // Diffuse term: clamped cosine, killed if the light is blocked. The
    // shadow ray is only worth casting when the surface faces the light at
    // all (cos > 0) — back-facing points are dark with or without occluders.
    let diffuse = match light {
        Some(l) => {
            let cos = hit.normal.dot(l).max(0.0);
            if cos > 0.0 && in_shadow(scene, hit.point, hit.normal, l) {
                0.0 // occluded: only the ambient floor survives
            } else {
                cos
            }
        }
        None => 0.0, // no light in the scene
    };
    let local = material.albedo * (scene.ambient + diffuse);

    // Reflection. Matte surfaces and exhausted depth return `local`
    // *unblended* — keep this early return so reflectivity 0 stays
    // bit-identical to plain Lambertian shading.
    if material.reflectivity <= 0.0 || depth == 0 {
        return local;
    }
    // Mirror the incoming direction about the normal: r = d − 2(d·n)n.
    // r·n = −(d·n) > 0 for a front-facing hit, so offsetting the origin
    // along +n (same acne bias as shadow rays) starts it on the open side.
    let d = ray.direction;
    let reflected_dir = d - hit.normal * (2.0 * d.dot(hit.normal));
    match Ray::new(hit.point + hit.normal * SHADOW_BIAS, reflected_dir) {
        Some(reflected_ray) => {
            let reflected = trace(scene, reflected_ray, light, depth - 1);
            local * (1.0 - material.reflectivity) + reflected * material.reflectivity
        }
        // Defensive only: mirroring a unit vector yields a unit vector, so
        // Ray::new cannot actually refuse it.
        None => local,
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

    /// One red matte unit sphere at the origin, lit from the camera's
    /// direction, zero ambient — the plain-Lambertian baseline scene.
    fn test_scene() -> Scene {
        Scene {
            spheres: vec![(
                Sphere::new(Vec3::ZERO, 1.0),
                Material::matte(LinearRgb::new(0.8, 0.1, 0.1)),
            )],
            light_dir: Vec3::Z, // toward the light, i.e. behind the camera
            background: LinearRgb::new(0.0, 0.0, 0.25),
            ambient: 0.0,
            max_depth: Scene::DEFAULT_MAX_DEPTH,
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
    fn material_new_clamps_reflectivity() {
        let c = LinearRgb::WHITE;
        assert!(approx_eq(Material::new(c, 0.4).reflectivity, 0.4));
        // Out-of-range values clamp instead of breaking the blend invariant.
        assert!(approx_eq(Material::new(c, 1.5).reflectivity, 1.0));
        assert!(approx_eq(Material::new(c, -0.5).reflectivity, 0.0));
        assert!(approx_eq(Material::matte(c).reflectivity, 0.0));
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
        let albedo = scene.spheres[0].1.albedo;
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
    fn render_without_light_leaves_only_ambient() {
        let mut scene = test_scene();
        scene.light_dir = Vec3::ZERO; // no direction → no light
        let camera = test_camera();
        let mut fb = Framebuffer::new(WIDTH, HEIGHT);

        // With zero ambient the old behavior holds: objects shade black.
        render(&scene, &camera, &mut fb);
        assert_eq!(fb.pixel(WIDTH / 2, HEIGHT / 2), Some(LinearRgb::BLACK));
        assert_eq!(fb.pixel(0, 0), Some(scene.background));

        // With an ambient floor, unlit objects show that fraction of albedo.
        scene.ambient = 0.1;
        render(&scene, &camera, &mut fb);
        let center = fb.pixel(WIDTH / 2, HEIGHT / 2).unwrap();
        let albedo = scene.spheres[0].1.albedo;
        assert!(approx_eq(center.r, albedo.r * 0.1));
        assert!(approx_eq(center.g, albedo.g * 0.1));
        assert!(approx_eq(center.b, albedo.b * 0.1));
    }

    #[test]
    fn nearest_sphere_wins() {
        // Two spheres on the view axis: the closer one must own the pixel.
        let scene = Scene {
            spheres: vec![
                (
                    Sphere::new(Vec3::new(0.0, 0.0, -3.0), 1.0),
                    Material::matte(LinearRgb::WHITE),
                ),
                (
                    Sphere::new(Vec3::ZERO, 1.0),
                    Material::matte(LinearRgb::new(1.0, 0.0, 0.0)),
                ),
            ],
            light_dir: Vec3::Z,
            background: LinearRgb::BLACK,
            ambient: 0.0,
            max_depth: Scene::DEFAULT_MAX_DEPTH,
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
    fn occluder_casts_shadow_leaving_only_ambient() {
        // Light comes from +Z (behind the camera). The blocker sits at
        // z = 8 — behind the camera at z = 5, so primary rays (travelling
        // toward −Z) can never see it, but every shadow ray from the front
        // of the target sphere toward +Z runs straight into it.
        let target = (
            Sphere::new(Vec3::ZERO, 1.0),
            Material::matte(LinearRgb::new(0.8, 0.7, 0.6)),
        );
        let blocker = (
            Sphere::new(Vec3::new(0.0, 0.0, 8.0), 1.0),
            Material::matte(LinearRgb::WHITE),
        );
        let mut scene = Scene {
            spheres: vec![target, blocker],
            light_dir: Vec3::Z,
            background: LinearRgb::BLACK,
            ambient: 0.1,
            max_depth: Scene::DEFAULT_MAX_DEPTH,
        };
        let camera = test_camera();

        let mut shadowed = Framebuffer::new(WIDTH, HEIGHT);
        render(&scene, &camera, &mut shadowed);
        let dark = shadowed.pixel(WIDTH / 2, HEIGHT / 2).unwrap();

        // Same scene without the blocker: the same pixel is fully lit.
        scene.spheres.pop();
        let mut lit = Framebuffer::new(WIDTH, HEIGHT);
        render(&scene, &camera, &mut lit);
        let bright = lit.pixel(WIDTH / 2, HEIGHT / 2).unwrap();

        // Occluded: only the ambient floor survives — exactly albedo·ambient.
        let albedo = scene.spheres[0].1.albedo;
        assert!(approx_eq(dark.r, albedo.r * 0.1), "dark.r = {}", dark.r);
        assert!(approx_eq(dark.g, albedo.g * 0.1));
        assert!(approx_eq(dark.b, albedo.b * 0.1));
        // And it is much darker than the unoccluded render (n·l ≈ 1 there).
        assert!(bright.r > dark.r * 5.0, "shadow did not darken the pixel");
    }

    #[test]
    fn reflective_sphere_picks_up_neighbor_color() {
        // A perfect mirror (black albedo, reflectivity 1) at the origin and
        // a big red matte sphere at z = 10, behind the camera. The center
        // primary ray arrives along ≈−Z, mirrors about the ≈+Z normal back
        // toward +Z, and hits the red sphere — so the mirror's pixel can
        // only be red if the reflected component is present. (The red
        // sphere is deliberately large: a convex mirror demagnifies, so a
        // small target would fall outside the reflected ray's reach.)
        let mirror = (
            Sphere::new(Vec3::ZERO, 1.0),
            Material::new(LinearRgb::BLACK, 1.0),
        );
        let red = (
            Sphere::new(Vec3::new(0.0, 0.0, 10.0), 4.0),
            Material::matte(LinearRgb::new(0.9, 0.0, 0.0)),
        );
        let mut scene = Scene {
            spheres: vec![mirror, red],
            light_dir: Vec3::Z,
            background: LinearRgb::BLACK,
            // The reflected face of the red sphere looks away from the light
            // (n·l < 0), so what the mirror sees is its ambient term.
            ambient: 0.2,
            max_depth: Scene::DEFAULT_MAX_DEPTH,
        };
        let camera = test_camera();
        let mut fb = Framebuffer::new(WIDTH, HEIGHT);
        render(&scene, &camera, &mut fb);
        let center = fb.pixel(WIDTH / 2, HEIGHT / 2).unwrap();
        assert!(
            center.r > 0.1,
            "reflected red component missing: {center:?}"
        );
        assert!(approx_eq(center.g, 0.0) && approx_eq(center.b, 0.0));

        // Turning reflectivity off removes exactly that component: the
        // black-albedo sphere shows only its own (black) ambient shade.
        scene.spheres[0].1 = Material::matte(LinearRgb::BLACK);
        render(&scene, &camera, &mut fb);
        let matte_center = fb.pixel(WIDTH / 2, HEIGHT / 2).unwrap();
        assert!(approx_eq(matte_center.r, 0.0));
    }

    #[test]
    fn zero_reflectivity_is_bit_identical_to_plain_lambertian() {
        // The editing-rule regression: with reflectivity 0 and ambient 0,
        // every pixel must equal the pre-shadow/pre-reflection tracer's
        // `albedo · max(n·l, 0)` **exactly** (an exact identity, so `==` is
        // allowed here). This pins the `reflectivity <= 0` early return.
        let scene = test_scene();
        let camera = test_camera();
        let mut fb = Framebuffer::new(WIDTH, HEIGHT);
        render(&scene, &camera, &mut fb);

        let (sphere, material) = scene.spheres[0];
        let light = scene.light_dir.normalize().unwrap();
        for py in 0..HEIGHT {
            for px in 0..WIDTH {
                let ray = camera.primary_ray(px, py, WIDTH, HEIGHT);
                let expected = match sphere.intersect(ray) {
                    Some(hit) => material.albedo * hit.normal.dot(light).max(0.0),
                    None => scene.background,
                };
                assert_eq!(fb.pixel(px, py), Some(expected), "pixel ({px}, {py})");
            }
        }
    }

    #[test]
    fn depth_limit_terminates_between_facing_mirrors() {
        // Two touching-distance perfect mirrors: rays grazing their inner
        // edges ping-pong between them, and with reflectivity 1 the light
        // never attenuates — only `max_depth` stops the recursion. The test
        // finishing at all is the proof; the assertions are sanity checks.
        let scene = Scene {
            spheres: vec![
                (
                    Sphere::new(Vec3::new(-1.05, 0.0, 0.0), 1.0),
                    Material::new(LinearRgb::WHITE, 1.0),
                ),
                (
                    Sphere::new(Vec3::new(1.05, 0.0, 0.0), 1.0),
                    Material::new(LinearRgb::WHITE, 1.0),
                ),
            ],
            light_dir: Vec3::new(0.0, 1.0, 1.0),
            background: LinearRgb::new(0.1, 0.1, 0.1),
            ambient: 0.05,
            max_depth: 8,
        };
        let camera = test_camera();
        let mut fb = Framebuffer::new(WIDTH, HEIGHT);
        render(&scene, &camera, &mut fb);
        // Every pixel came out finite — the recursion bottomed out cleanly.
        assert!(fb
            .pixels
            .iter()
            .all(|p| p.r.is_finite() && p.g.is_finite() && p.b.is_finite()));
    }

    #[test]
    fn lit_hemisphere_has_no_shadow_acne() {
        // Regression for the SHADOW_BIAS offset: a lone sphere must never
        // occlude itself. Every pixel whose surface clearly faces the light
        // has to keep its diffuse term — without the bias, rounding error in
        // the hit point makes random lit pixels re-hit their own sphere and
        // drop to the ambient floor (speckle).
        let scene = test_scene(); // one sphere, ambient 0, light = +Z
        let camera = test_camera();
        let mut fb = Framebuffer::new(WIDTH, HEIGHT);
        render(&scene, &camera, &mut fb);

        let light = scene.light_dir.normalize().unwrap();
        let (sphere, material) = scene.spheres[0];
        let mut checked = 0;
        for py in 0..HEIGHT {
            for px in 0..WIDTH {
                let ray = camera.primary_ray(px, py, WIDTH, HEIGHT);
                if let Some(hit) = sphere.intersect(ray) {
                    let cos = hit.normal.dot(light);
                    if cos > 0.05 {
                        // Clearly lit: the pixel must show diffuse light,
                        // not the black an acne self-shadow would leave.
                        let p = fb.pixel(px, py).unwrap();
                        assert!(
                            p.r > material.albedo.r * cos * 0.5,
                            "acne speckle at ({px}, {py}): {p:?}"
                        );
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 50, "test lost its subject: {checked} lit pixels");
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
