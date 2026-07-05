//! N-body orbit animation: `simulation` computes the physics, `render`
//! draws it — proof that two domain crates compose through public APIs.
//!
//! `simulation` is a **dev-dependency** here, sanctioned by the root
//! AGENTS.md exception: examples may showcase crates composing, while
//! library `[dependencies]` between domain crates stay forbidden.
//!
//! Each frame: advance the 2D velocity-Verlet integrator
//! ([`simulation::physics::step_nbody`]), lift the bodies from the `z = 0`
//! plane into 3D spheres, and ray-trace them through [`render::Camera`].
//! Frames land in `target/nbody-frames/` as PPM files (`target/` is
//! gitignored, so the output never pollutes the repository).
//!
//! Run with: `cargo run -p render --example nbody --release`

use std::fs;
use std::path::Path;

use math::Vec3;
use render::{render, Camera, Framebuffer, LinearRgb, Material, Scene, Sphere};
use simulation::physics::{step_nbody, Body, Vec2};

/// Image size: small enough that 90 software-traced frames finish in
/// seconds, large enough that the orbits read clearly.
const WIDTH: usize = 160;
/// Image height (4:3, matching the camera's aspect ratio).
const HEIGHT: usize = 120;
/// Number of frames — the inner planet completes roughly one full orbit.
const FRAMES: usize = 90;
/// Physics substeps per frame: several small Verlet steps keep the orbit
/// stable while the *frame* still advances far enough to see motion.
const SUBSTEPS: usize = 4;
/// Physics time step per substep.
const DT: f64 = 0.008;
/// Gravitational constant of this toy universe.
const G: f64 = 1.0;
/// Softening length: prevents the force singularity at close encounters.
const SOFTENING: f64 = 1.0;

/// Sun + two planets on near-circular orbits (v = √(G·M/r)), orbiting in
/// opposite directions so the motion is unmistakable.
fn initial_bodies() -> Vec<Body> {
    let sun_mass = 1.0e6;
    vec![
        Body {
            pos: Vec2::ZERO,
            vel: Vec2::ZERO,
            mass: sun_mass,
        },
        Body {
            // Inner planet, counter-clockwise: r = 60 → v ≈ 129.1.
            pos: Vec2::new(60.0, 0.0),
            vel: Vec2::new(0.0, (G * sun_mass / 60.0).sqrt()),
            mass: 1.0,
        },
        Body {
            // Outer planet, clockwise: r = 100 → v = 100.
            pos: Vec2::new(-100.0, 0.0),
            vel: Vec2::new(0.0, (G * sun_mass / 100.0).sqrt()),
            mass: 1.0,
        },
    ]
}

/// Visual radius and material for body `i` (sun first, then planets).
/// Radii are fixed per body — mass-proportional sizing would make the
/// planets sub-pixel next to a 10⁶-mass sun.
fn appearance(i: usize) -> (f64, Material) {
    match i {
        // The sun: big, warm, matte.
        0 => (14.0, Material::matte(LinearRgb::new(1.0, 0.85, 0.4))),
        // Inner planet: blue, slightly mirror-like (shows off reflection).
        1 => (6.0, Material::new(LinearRgb::new(0.2, 0.4, 0.9), 0.25)),
        // Outer planet: red.
        _ => (8.0, Material::new(LinearRgb::new(0.9, 0.25, 0.15), 0.25)),
    }
}

/// Build the render scene for the current body positions: each 2D body in
/// the `z = 0` plane becomes a sphere (simulation's `Vec2` → math's `Vec3`).
fn scene_from_bodies(bodies: &[Body]) -> Scene {
    Scene {
        spheres: bodies
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let (radius, material) = appearance(i);
                (
                    Sphere::new(Vec3::new(b.pos.x, b.pos.y, 0.0), radius),
                    material,
                )
            })
            .collect(),
        light_dir: Vec3::new(0.3, 0.5, 1.0),
        background: LinearRgb::new(0.01, 0.01, 0.03), // near-black space
        ambient: 0.08,
        max_depth: Scene::DEFAULT_MAX_DEPTH,
    }
}

fn main() {
    // `target/` is gitignored — frames never end up in version control.
    let out_dir = Path::new("target/nbody-frames");
    fs::create_dir_all(out_dir).expect("create target/nbody-frames");

    let camera = Camera::new(
        Vec3::new(0.0, 0.0, 260.0), // far enough back to frame the r=100 orbit
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
    let start = std::time::Instant::now();

    for frame in 0..FRAMES {
        for _ in 0..SUBSTEPS {
            step_nbody(&mut bodies, DT, G, SOFTENING);
        }
        render(&scene_from_bodies(&bodies), &camera, &mut fb);
        let path = out_dir.join(format!("frame_{frame:03}.ppm"));
        fs::write(&path, fb.to_ppm()).expect("write frame");
        if frame % 15 == 0 || frame == FRAMES - 1 {
            println!("frame {:>3}/{FRAMES} -> {}", frame + 1, path.display());
        }
    }

    println!(
        "\nrendered {FRAMES} frames ({WIDTH}x{HEIGHT}) in {:.2?}",
        start.elapsed()
    );
    println!("turn them into a video or GIF with one of:");
    println!(
        "  ffmpeg -framerate 30 -i target/nbody-frames/frame_%03d.ppm -pix_fmt yuv420p nbody.mp4"
    );
    println!("  magick -delay 3 target/nbody-frames/frame_*.ppm nbody.gif");
}
