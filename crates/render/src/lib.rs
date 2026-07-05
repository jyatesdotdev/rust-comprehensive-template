//! The geometry of rendering: how a 3D point becomes a pixel, and how a
//! pixel becomes a ray — every step in plain Rust, built on the [`math`]
//! crate and nothing else.
//!
//! # The point-to-pixel pipeline
//!
//! Every renderer, GPU or CPU, runs the same chain of spaces:
//!
//! ```text
//! model ──model matrix──▶ world ──view──▶ view ──projection + ÷w──▶ NDC ──viewport──▶ screen
//!        (math::Transform)      (look_at_rh)   (perspective_rh)   [-1,1]³  (Y flip)  (px, py)
//! ```
//!
//! - **model → world**: place the object ([`math::Transform`] baking `T·R·S`).
//! - **world → view**: re-express everything relative to the camera, which
//!   sits at the origin looking down **−Z** ([`math::look_at_rh`]).
//! - **view → clip → NDC**: [`math::perspective_rh`] plus the divide by `w`
//!   squeezes the viewing frustum into the `[-1, 1]³` cube; this divide is
//!   what makes distant things small.
//! - **NDC → screen**: the viewport transform maps `[-1, 1]` onto pixel
//!   coordinates, flipping Y because images put row 0 at the top
//!   ([`Camera::ndc_to_screen`]).
//!
//! And the **inverse pipeline** is ray tracing: start from a pixel, undo the
//! viewport transform and the projection, and you get a world-space ray
//! through that pixel ([`Camera::primary_ray`]); intersect it with geometry
//! ([`Sphere`], [`Aabb`], [`Plane`]) and shade in linear light
//! ([`LinearRgb`]). Both directions are the same math — the camera module's
//! round-trip tests prove it. On top of the primary rays, [`raytrace`] casts
//! two kinds of *secondary* rays: shadow rays (is the light visible from the
//! hit point?) and one-bounce reflection rays (mirror materials, recursion
//! bounded by [`Scene::max_depth`]).
//!
//! The `nbody` example (`cargo run -p render --example nbody`) drives the
//! whole crate from the `simulation` crate's velocity-Verlet N-body
//! integrator — two domain crates composing through their public APIs.
//!
//! # Which parts the GPU normally owns
//!
//! In a production stack you write only the matrices and the shading; the
//! hardware does the rest. With [`wgpu`](https://docs.rs/wgpu): the vertex
//! shader outputs clip-space positions (your `proj * view * model`), then
//! **fixed-function hardware** performs the perspective divide, clipping,
//! the viewport transform, and rasterization; the fragment shader shades;
//! the `Srgb` surface format applies the transfer function on write. The
//! math itself is [`glam`](https://docs.rs/glam) (f32, SIMD) and image I/O
//! is the [`image`](https://docs.rs/image) crate — this crate's PPM writer
//! and f64 pipeline exist so every intermediate value stays inspectable.
//!
//! ```rust
//! use math::Vec3;
//! use render::{render, Camera, Framebuffer, LinearRgb, Material, Scene, Sphere};
//!
//! let camera = Camera::new(
//!     Vec3::new(0.0, 0.0, 5.0), // eye
//!     Vec3::ZERO,               // target
//!     Vec3::Y,                  // up
//!     std::f64::consts::FRAC_PI_3,
//!     4.0 / 3.0,
//!     0.1,
//!     100.0,
//! )
//! .expect("camera parameters are non-degenerate");
//! let scene = Scene {
//!     spheres: vec![(
//!         Sphere::new(Vec3::ZERO, 1.0),
//!         Material::new(LinearRgb::new(0.8, 0.1, 0.1), 0.2), // slightly mirror-like
//!     )],
//!     light_dir: Vec3::new(1.0, 1.0, 1.0),
//!     background: LinearRgb::new(0.0, 0.0, 0.2),
//!     ambient: 0.05,
//!     max_depth: Scene::DEFAULT_MAX_DEPTH,
//! };
//! let mut fb = Framebuffer::new(16, 12);
//! render(&scene, &camera, &mut fb);
//! assert!(fb.to_ppm().starts_with("P3\n16 12\n255\n"));
//! ```

pub mod camera;
pub mod color;
pub mod geometry;
pub mod raytrace;

// Re-export the main types so dependents can write `use render::{Camera, Ray}`.
pub use camera::Camera;
pub use color::LinearRgb;
pub use geometry::{Aabb, Hit, Plane, Ray, Sphere};
pub use raytrace::{render, Framebuffer, Material, Scene};

/// Shared float-comparison helpers for this crate's tests (same rationale as
/// `math`'s `approx_eq`: never compare computed floats with `==`).
#[cfg(test)]
pub(crate) mod test_util {
    use math::Vec3;

    /// Absolute-difference comparison, matching `math`'s test tolerance.
    pub(crate) fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// Componentwise [`approx_eq`] for vectors.
    pub(crate) fn v3_approx_eq(a: Vec3, b: Vec3) -> bool {
        approx_eq(a.x, b.x) && approx_eq(a.y, b.y) && approx_eq(a.z, b.z)
    }
}
