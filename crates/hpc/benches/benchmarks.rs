//! Criterion benchmarks for HPC operations.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use hpc::{parallel, simd, zero_cost};

fn bench_dot_product(c: &mut Criterion) {
    let a: Vec<f32> = (0..1024).map(|i| i as f32).collect();
    let b: Vec<f32> = (0..1024).map(|i| (i * 2) as f32).collect();

    c.bench_function("dot_product_scalar", |bencher| {
        bencher.iter(|| simd::dot_product(black_box(&a), black_box(&b)))
    });

    #[cfg(target_arch = "x86_64")]
    c.bench_function("dot_product_sse", |bencher| {
        bencher.iter(|| unsafe { simd::dot_product_sse(black_box(&a), black_box(&b)) })
    });
}

fn bench_parallel_sort(c: &mut Criterion) {
    c.bench_function("parallel_sort_10k", |bencher| {
        bencher.iter_batched(
            || (0..10_000).rev().collect::<Vec<i32>>(),
            |mut data| parallel::parallel_sort(&mut data),
            criterion::BatchSize::SmallInput,
        )
    });
}

fn bench_generic_sum(c: &mut Criterion) {
    let data: Vec<f64> = (0..10_000).map(|i| i as f64).collect();
    c.bench_function("generic_sum_f64_10k", |bencher| {
        bencher.iter(|| zero_cost::generic_sum(black_box(&data)))
    });
}

criterion_group!(benches, bench_dot_product, bench_parallel_sort, bench_generic_sum);
criterion_main!(benches);
