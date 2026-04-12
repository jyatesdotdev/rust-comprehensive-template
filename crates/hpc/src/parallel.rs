//! Rayon parallel iterators: parallel map, reduce, sort, and custom work-stealing.

use rayon::prelude::*;

/// Parallel map-reduce: sum of squares over a slice.
pub fn parallel_sum_of_squares(data: &[f64]) -> f64 {
    data.par_iter().map(|x| x * x).sum()
}

/// Parallel sort (unstable, in-place).
pub fn parallel_sort<T: Send + Ord>(data: &mut [T]) {
    data.par_sort_unstable();
}

/// Parallel filter-map: extract and transform matching elements.
pub fn parallel_filter_map<T, U, F>(data: &[T], f: F) -> Vec<U>
where
    T: Sync,
    U: Send,
    F: Fn(&T) -> Option<U> + Sync + Send,
{
    data.par_iter().filter_map(f).collect()
}

/// Parallel fold + reduce: chunked aggregation with custom combiner.
/// Each chunk folds independently, then results are reduced together.
pub fn parallel_fold_reduce<T, Init, Fold, Reduce>(
    data: &[T],
    identity: Init,
    fold_op: Fold,
    reduce_op: Reduce,
) -> f64
where
    T: Sync,
    Init: Fn() -> f64 + Sync,
    Fold: Fn(f64, &T) -> f64 + Sync,
    Reduce: Fn(f64, f64) -> f64 + Sync,
{
    data.par_iter()
        .fold(&identity, |acc, item| fold_op(acc, item))
        .reduce(&identity, |a, b| reduce_op(a, b))
}

/// Configure a custom thread pool and run work inside it.
pub fn with_thread_pool<F, R>(num_threads: usize, f: F) -> R
where
    F: FnOnce() -> R + Send,
    R: Send,
{
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
        .expect("failed to build rayon thread pool");
    pool.install(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sum_of_squares() {
        let data = vec![1.0, 2.0, 3.0, 4.0];
        assert!((parallel_sum_of_squares(&data) - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn sort() {
        let mut data = vec![5, 3, 1, 4, 2];
        parallel_sort(&mut data);
        assert_eq!(data, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn filter_map() {
        let data: Vec<i32> = (0..10).collect();
        let evens: Vec<i32> = parallel_filter_map(&data, |x| {
            if x % 2 == 0 { Some(x * 10) } else { None }
        });
        assert_eq!(evens.len(), 5);
        assert!(evens.contains(&0));
        assert!(evens.contains(&40));
    }

    #[test]
    fn fold_reduce() {
        let data = vec![1.0, 2.0, 3.0, 4.0];
        let result = parallel_fold_reduce(&data, || 0.0, |acc, x| acc + x, |a, b| a + b);
        assert!((result - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn custom_thread_pool() {
        let result = with_thread_pool(2, || {
            let data: Vec<i32> = (0..100).collect();
            data.par_iter().sum::<i32>()
        });
        assert_eq!(result, 4950);
    }
}
