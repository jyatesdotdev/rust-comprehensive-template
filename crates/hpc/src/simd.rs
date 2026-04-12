//! SIMD-friendly computation patterns.
//!
//! Demonstrates auto-vectorizable code and manual SIMD intrinsics.
//! The scalar versions are written in a style the compiler can auto-vectorize
//! with `opt-level = 3`.

/// Dot product written for auto-vectorization (no manual intrinsics needed).
pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Element-wise vector addition into `out`. Auto-vectorizes at opt-level 3.
pub fn vec_add(a: &[f32], b: &[f32], out: &mut [f32]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), out.len());
    for i in 0..a.len() {
        out[i] = a[i] + b[i];
    }
}

/// Manual SIMD dot product using x86 SSE intrinsics (128-bit, 4×f32).
///
/// # Safety
/// Caller must ensure the CPU supports SSE (virtually all x86_64 CPUs do).
#[cfg(target_arch = "x86_64")]
pub unsafe fn dot_product_sse(a: &[f32], b: &[f32]) -> f32 {
    use std::arch::x86_64::*;
    assert_eq!(a.len(), b.len());

    let chunks = a.len() / 4;
    let mut acc = _mm_setzero_ps();

    for i in 0..chunks {
        let offset = i * 4;
        let va = _mm_loadu_ps(a.as_ptr().add(offset));
        let vb = _mm_loadu_ps(b.as_ptr().add(offset));
        acc = _mm_add_ps(acc, _mm_mul_ps(va, vb));
    }

    // Horizontal sum of the 4 lanes.
    let mut result = [0.0f32; 4];
    _mm_storeu_ps(result.as_mut_ptr(), acc);
    let mut sum: f32 = result.iter().sum();

    // Handle remainder elements.
    for i in (chunks * 4)..a.len() {
        sum += a[i] * b[i];
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dot_product() {
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![5.0, 6.0, 7.0, 8.0];
        assert!((dot_product(&a, &b) - 70.0).abs() < 1e-6);
    }

    #[test]
    fn test_vec_add() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let mut out = vec![0.0; 3];
        vec_add(&a, &b, &mut out);
        assert_eq!(out, vec![5.0, 7.0, 9.0]);
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_dot_product_sse() {
        let a: Vec<f32> = (0..10).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..10).map(|i| (i * 2) as f32).collect();
        let expected = dot_product(&a, &b);
        let sse_result = unsafe { dot_product_sse(&a, &b) };
        assert!((sse_result - expected).abs() < 1e-3);
    }
}
