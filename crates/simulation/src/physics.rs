//! Physics simulation: 2D vectors, Verlet integration, N-body gravity.

use std::ops::{Add, Mul, Sub};

/// 2D vector used throughout physics simulations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2 {
    /// X component.
    pub x: f64,
    /// Y component.
    pub y: f64,
}

impl Vec2 {
    /// The zero vector.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// Create a new vector from `x` and `y` components.
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Euclidean length (magnitude) of the vector.
    pub fn length(self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
}

impl Add for Vec2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Vec2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Mul<f64> for Vec2 {
    type Output = Self;
    fn mul(self, s: f64) -> Self {
        Self {
            x: self.x * s,
            y: self.y * s,
        }
    }
}

/// A body with position, velocity, and mass.
#[derive(Debug, Clone)]
pub struct Body {
    /// Current position.
    pub pos: Vec2,
    /// Current velocity.
    pub vel: Vec2,
    /// Mass of the body.
    pub mass: f64,
}

/// Compute the gravitational acceleration acting on each body.
fn accelerations(bodies: &[Body], g: f64, softening: f64) -> Vec<Vec2> {
    let n = bodies.len();
    let mut accel = vec![Vec2::ZERO; n];
    for i in 0..n {
        for j in (i + 1)..n {
            let r = bodies[j].pos - bodies[i].pos;
            let dist2 = r.x * r.x + r.y * r.y + softening * softening;
            let inv_dist3 = 1.0 / (dist2 * dist2.sqrt());
            // Force on i from j; equal and opposite on j (Newton's third law),
            // so each pair is computed once.
            let fi = r * (g * bodies[i].mass * bodies[j].mass * inv_dist3);
            accel[i] = accel[i] + fi * (1.0 / bodies[i].mass);
            accel[j] = accel[j] - fi * (1.0 / bodies[j].mass);
        }
    }
    accel
}

/// Advance N-body system one step using velocity-Verlet integration.
///
/// Velocity-Verlet is symplectic: unlike naive Euler, its energy error stays
/// bounded over long runs, so orbits neither spiral in nor fly apart.
/// `g` is the gravitational constant, `dt` the time step, and `softening`
/// prevents the force singularity when two bodies get arbitrarily close.
pub fn step_nbody(bodies: &mut [Body], dt: f64, g: f64, softening: f64) {
    // 1. Drift: x(t+dt) = x(t) + v(t)·dt + ½·a(t)·dt²
    let a0 = accelerations(bodies, g, softening);
    for (body, &acc) in bodies.iter_mut().zip(&a0) {
        body.pos = body.pos + body.vel * dt + acc * (0.5 * dt * dt);
    }
    // 2. Kick: v(t+dt) = v(t) + ½·(a(t) + a(t+dt))·dt — requires the
    //    accelerations at the *new* positions, hence the second pass.
    let a1 = accelerations(bodies, g, softening);
    for (body, (&acc0, &acc1)) in bodies.iter_mut().zip(a0.iter().zip(&a1)) {
        body.vel = body.vel + (acc0 + acc1) * (0.5 * dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec2_ops() {
        let a = Vec2::new(3.0, 4.0);
        assert!((a.length() - 5.0).abs() < 1e-10);
        let b = a + Vec2::new(1.0, -1.0);
        assert_eq!(b, Vec2::new(4.0, 3.0));
    }

    #[test]
    fn two_body_orbit() {
        let mut bodies = vec![
            Body {
                pos: Vec2::ZERO,
                vel: Vec2::ZERO,
                mass: 1e6,
            },
            Body {
                pos: Vec2::new(100.0, 0.0),
                vel: Vec2::new(0.0, 50.0),
                mass: 1.0,
            },
        ];
        // Run 1000 steps — satellite should stay roughly the same distance.
        let initial_dist = (bodies[1].pos - bodies[0].pos).length();
        for _ in 0..1000 {
            step_nbody(&mut bodies, 0.01, 1.0, 1.0);
        }
        let final_dist = (bodies[1].pos - bodies[0].pos).length();
        // Orbit should keep the body in a bounded region (not escape or collapse).
        assert!(final_dist > initial_dist * 0.1, "body collapsed");
        assert!(final_dist < initial_dist * 10.0, "body escaped");
    }
}
