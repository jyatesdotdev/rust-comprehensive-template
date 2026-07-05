//! Linear-light color and the sRGB transfer function: [`LinearRgb`].
//!
//! # Why linear vs. gamma matters
//!
//! Physical light adds linearly: two equal lamps produce exactly twice the
//! photons of one. Every lighting computation — `n·l` shading, averaging,
//! blending — is a statement about photons, so it is **only correct in a
//! linear space**. But displays (and 8-bit image files) use the *sRGB*
//! encoding, a roughly-power-2.2 curve chosen because human vision is more
//! sensitive to dark steps than bright ones: spending the 256 codes evenly
//! in linear light would waste most of them on highlights nobody can tell
//! apart and visibly band the shadows.
//!
//! The rule this module enforces by construction: **compute in
//! [`LinearRgb`] (f64, linear), and apply the sRGB encoding exactly once, at
//! the very end**, in [`LinearRgb::to_srgb_u8`]. Doing math on
//! gamma-encoded values (the classic mistake) makes 0.5 + 0.5 ≠ 1.0 in
//! photon terms — midtones come out too dark and colored edges fringe.
//!
//! Production equivalent: GPU pipelines do the same thing with
//! `Rgba8UnormSrgb` textures/surfaces in `wgpu` — the hardware applies this
//! exact transfer function on write. The `image` crate then owns the file
//! encoding.

/// A color in **linear** light, one `f64` per channel.
///
/// `0.0` is black and `1.0` is display white, but values are *not* clamped:
/// intermediate results of lighting math may exceed 1 (a bright light) or
/// be summed/scaled freely. Clamping happens once, in
/// [`LinearRgb::to_srgb_u8`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LinearRgb {
    /// Red channel, linear light.
    pub r: f64,
    /// Green channel, linear light.
    pub g: f64,
    /// Blue channel, linear light.
    pub b: f64,
}

impl LinearRgb {
    /// Pure black (all zeros).
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
    };
    /// Display white (all ones).
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
    };

    /// Create a color from linear-light channels.
    pub fn new(r: f64, g: f64, b: f64) -> Self {
        Self { r, g, b }
    }

    /// Encode to 8-bit sRGB: clamp to `[0, 1]`, apply the transfer
    /// function, then quantize to `0..=255`.
    ///
    /// This is the **real** piecewise sRGB curve (IEC 61966-2-1), not the
    /// `x^(1/2.2)` approximation: a short linear segment below 0.0031308
    /// (a pure power curve has infinite slope at 0, which would make the
    /// darkest codes numerically unstable and non-invertible in practice),
    /// then `1.055·x^(1/2.4) − 0.055`. The two pieces meet with matching
    /// value where they join. Known anchors: `0.0 → 0`, `1.0 → 255`, and
    /// linear `0.5 → 188` (not 128 — half the photons is much brighter than
    /// half the code range, which is the whole point of the encoding).
    ///
    /// Clamping happens *before* quantization so out-of-range light
    /// saturates cleanly instead of wrapping.
    pub fn to_srgb_u8(self) -> [u8; 3] {
        [
            srgb_encode_channel(self.r),
            srgb_encode_channel(self.g),
            srgb_encode_channel(self.b),
        ]
    }
}

/// One channel of the sRGB opto-electronic transfer function
/// (see [`LinearRgb::to_srgb_u8`]).
fn srgb_encode_channel(linear: f64) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let encoded = if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    // encoded is in [0, 1]; round to the nearest of the 256 codes.
    (encoded * 255.0).round() as u8
}

/// Adding light: photons from two sources accumulate (only meaningful in
/// linear space — the module docs explain why).
impl std::ops::Add for LinearRgb {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.r + rhs.r, self.g + rhs.g, self.b + rhs.b)
    }
}

/// Scaling light by intensity, e.g. `albedo * (n·l)` in Lambertian shading.
impl std::ops::Mul<f64> for LinearRgb {
    type Output = Self;
    fn mul(self, s: f64) -> Self {
        Self::new(self.r * s, self.g * s, self.b * s)
    }
}

/// Componentwise product: a surface reflecting colored light — each channel
/// of the surface's albedo filters the matching channel of the light.
impl std::ops::Mul for LinearRgb {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self::new(self.r * rhs.r, self.g * rhs.g, self.b * rhs.b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::approx_eq;

    #[test]
    fn srgb_known_values() {
        // The anchors every sRGB implementation must reproduce.
        assert_eq!(LinearRgb::BLACK.to_srgb_u8(), [0, 0, 0]);
        assert_eq!(LinearRgb::WHITE.to_srgb_u8(), [255, 255, 255]);
        // Half the photons is far brighter than half the code range.
        assert_eq!(LinearRgb::new(0.5, 0.5, 0.5).to_srgb_u8(), [188, 188, 188]);
    }

    #[test]
    fn srgb_linear_segment_near_black() {
        // Below the 0.0031308 knee the curve is linear: 12.92 · c.
        // 0.001 → 12.92 · 0.001 · 255 = 3.29 → 3.
        assert_eq!(LinearRgb::new(0.001, 0.0, 0.0).to_srgb_u8(), [3, 0, 0]);
        // Just above the knee, the power branch takes over continuously.
        let below = LinearRgb::new(0.003_130_8, 0.0, 0.0).to_srgb_u8()[0];
        let above = LinearRgb::new(0.003_140_0, 0.0, 0.0).to_srgb_u8()[0];
        assert!(above == below || above == below + 1);
    }

    #[test]
    fn srgb_clamps_before_quantizing() {
        // Overbright light saturates at white; negative light at black.
        assert_eq!(LinearRgb::new(2.0, 1.5, 100.0).to_srgb_u8(), [255; 3]);
        assert_eq!(LinearRgb::new(-1.0, -0.001, 0.0).to_srgb_u8(), [0; 3]);
    }

    #[test]
    fn add_accumulates_light() {
        let sum = LinearRgb::new(0.1, 0.2, 0.3) + LinearRgb::new(0.4, 0.5, 0.6);
        assert!(approx_eq(sum.r, 0.5));
        assert!(approx_eq(sum.g, 0.7));
        assert!(approx_eq(sum.b, 0.9));
    }

    #[test]
    fn mul_scales_and_filters() {
        let c = LinearRgb::new(0.2, 0.4, 0.8) * 0.5;
        assert!(approx_eq(c.r, 0.1) && approx_eq(c.g, 0.2) && approx_eq(c.b, 0.4));
        // A red surface under white light reflects only red.
        let lit = LinearRgb::new(1.0, 0.0, 0.0) * LinearRgb::WHITE;
        assert!(approx_eq(lit.r, 1.0) && approx_eq(lit.g, 0.0) && approx_eq(lit.b, 0.0));
    }

    #[test]
    fn default_is_black() {
        assert_eq!(LinearRgb::default(), LinearRgb::BLACK);
    }
}
