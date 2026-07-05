//! Integration test for the `nbody` example's core loop: `simulation`
//! stepping bodies, `render` drawing them — entirely in memory, no file
//! I/O. This keeps the simulate→render composition under the coverage
//! gate even though the example binary itself is not instrumented.
//!
//! `simulation` is a dev-dependency, sanctioned by the root AGENTS.md
//! exception for examples/tests that demonstrate crates composing.

use math::Vec3;
use render::{render, Camera, Framebuffer, LinearRgb, Material, Scene, Sphere};
use simulation::physics::{step_nbody, Body, Vec2};

const WIDTH: usize = 32;
const HEIGHT: usize = 24;
const BACKGROUND: LinearRgb = LinearRgb::BLACK;

/// Sun + one planet on a near-circular orbit (v = √(G·M/r)), the same
/// setup as `examples/nbody.rs` at test scale. Fully deterministic: plain
/// f64 arithmetic, no RNG, no threads.
fn initial_bodies() -> Vec<Body> {
    let sun_mass = 1.0e6;
    vec![
        Body {
            pos: Vec2::ZERO,
            vel: Vec2::ZERO,
            mass: sun_mass,
        },
        Body {
            pos: Vec2::new(60.0, 0.0),
            vel: Vec2::new(0.0, (sun_mass / 60.0_f64).sqrt()),
            mass: 1.0,
        },
    ]
}

/// Lift the 2D bodies (z = 0 plane) into render's 3D scene, exactly like
/// the example does: simulation's `Vec2` becomes math's `Vec3`.
fn scene_from_bodies(bodies: &[Body]) -> Scene {
    let radii = [14.0, 8.0];
    Scene {
        spheres: bodies
            .iter()
            .zip(radii)
            .map(|(b, radius)| {
                (
                    Sphere::new(Vec3::new(b.pos.x, b.pos.y, 0.0), radius),
                    Material::matte(LinearRgb::WHITE),
                )
            })
            .collect(),
        light_dir: Vec3::Z,
        background: BACKGROUND,
        ambient: 0.1,
        max_depth: Scene::DEFAULT_MAX_DEPTH,
    }
}

/// Indices of pixels that are not the background — i.e. where a body is.
fn lit_pixels(fb: &Framebuffer) -> Vec<usize> {
    fb.pixels
        .iter()
        .enumerate()
        .filter(|(_, &p)| p != BACKGROUND)
        .map(|(i, _)| i)
        .collect()
}

#[test]
fn simulated_bodies_visibly_move_between_rendered_frames() {
    let camera = Camera::new(
        Vec3::new(0.0, 0.0, 260.0),
        Vec3::ZERO,
        Vec3::Y,
        std::f64::consts::FRAC_PI_3,
        WIDTH as f64 / HEIGHT as f64,
        0.1,
        1000.0,
    )
    .expect("camera parameters are non-degenerate");

    let mut bodies = initial_bodies();
    let mut fb = Framebuffer::new(WIDTH, HEIGHT);

    // Frame 0.
    render(&scene_from_bodies(&bodies), &camera, &mut fb);
    let frame0 = lit_pixels(&fb);
    assert!(!frame0.is_empty(), "frame 0 shows no bodies at all");

    // Advance the physics far enough for the planet to cross pixels
    // (arc length ≈ v·t ≈ 129 · 0.32 ≈ 41 world units, several pixels
    // at this resolution), rendering each intermediate frame like the
    // example's loop does.
    let mut frame_n = frame0.clone();
    for _ in 0..2 {
        for _ in 0..20 {
            step_nbody(&mut bodies, 0.008, 1.0, 1.0);
        }
        render(&scene_from_bodies(&bodies), &camera, &mut fb);
        frame_n = lit_pixels(&fb);
    }

    assert!(!frame_n.is_empty(), "final frame shows no bodies at all");
    // Motion is visible: the set of lit pixels changed between frames.
    assert_ne!(
        frame0, frame_n,
        "planet did not move across any pixel between frames"
    );
}
