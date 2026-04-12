//! Criterion benchmarks for the testing crate.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use testing::{collections::SortedSet, math};

fn bench_fibonacci(c: &mut Criterion) {
    let mut group = c.benchmark_group("fibonacci");
    for n in [10, 20, 40, 60] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| math::fibonacci(black_box(n)))
        });
    }
    group.finish();
}

fn bench_gcd(c: &mut Criterion) {
    c.bench_function("gcd(46368, 28657)", |b| {
        b.iter(|| math::gcd(black_box(46368), black_box(28657)))
    });
}

fn bench_sorted_set_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("sorted_set_insert");
    for size in [100, 1_000, 10_000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                let mut set = SortedSet::new();
                for i in (0..size).rev() {
                    set.insert(black_box(i));
                }
                set
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_fibonacci, bench_gcd, bench_sorted_set_insert);
criterion_main!(benches);
