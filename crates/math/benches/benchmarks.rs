//! Criterion benchmarks for the `math` crate's hot paths.
//!
//! These measure the three operations a renderer runs per-object, per-frame:
//! Mat4 × Mat4 (building MVP matrices), Vec3 normalize (lighting), and
//! quaternion slerp (animation blending). The numbers make the cost of the
//! hand-rolled f64 code observable next to what a SIMD f32 library like
//! `glam` would report.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use math::{Quat, Vec3};

fn bench_mat4_mul(c: &mut Criterion) {
    // Realistic operands: a projection and a view matrix, as in an MVP chain.
    let proj = math::perspective_rh(1.0, 16.0 / 9.0, 0.1, 100.0);
    let view = math::look_at_rh(Vec3::new(3.0, 4.0, 5.0), Vec3::ZERO, Vec3::Y)
        .expect("non-degenerate camera");
    c.bench_function("mat4_mul", |bencher| {
        bencher.iter(|| black_box(proj) * black_box(view))
    });
}

fn bench_vec3_normalize(c: &mut Criterion) {
    let v = Vec3::new(3.0, -4.0, 12.0);
    c.bench_function("vec3_normalize", |bencher| {
        bencher.iter(|| black_box(v).normalize())
    });
}

fn bench_quat_slerp(c: &mut Criterion) {
    // A large angle so the real slerp path runs, not the nlerp fallback.
    let a = Quat::from_axis_angle(Vec3::Y, 0.1);
    let b = Quat::from_axis_angle(Vec3::new(1.0, 1.0, 0.0), 2.0);
    c.bench_function("quat_slerp", |bencher| {
        bencher.iter(|| black_box(a).slerp(black_box(b), black_box(0.3)))
    });
}

criterion_group!(
    benches,
    bench_mat4_mul,
    bench_vec3_normalize,
    bench_quat_slerp
);
criterion_main!(benches);
