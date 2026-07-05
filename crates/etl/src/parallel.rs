//! Rayon-based parallel batch ETL processing.
//!
//! Demonstrates parallel map-reduce patterns for large-scale data processing.

use rayon::prelude::*;
use std::collections::HashMap;
use std::hash::Hash;

/// Parallel map-reduce: apply `map_fn` to each item, then reduce with `reduce_fn`.
pub fn par_map_reduce<T, R, MapFn, ReduceFn>(
    data: &[T],
    identity: R,
    map_fn: MapFn,
    reduce_fn: ReduceFn,
) -> R
where
    T: Sync,
    R: Send + Clone + Sync,
    MapFn: Fn(&T) -> R + Sync + Send,
    ReduceFn: Fn(R, R) -> R + Sync,
{
    data.par_iter()
        .map(map_fn)
        .reduce(|| identity.clone(), &reduce_fn)
}

/// Parallel group-by: partition items by key, sum values per group.
pub fn par_group_sum<T, K, KeyFn, ValFn>(
    data: &[T],
    key_fn: KeyFn,
    val_fn: ValFn,
) -> HashMap<K, f64>
where
    T: Sync,
    K: Eq + Hash + Send,
    KeyFn: Fn(&T) -> K + Sync,
    ValFn: Fn(&T) -> f64 + Sync,
{
    data.par_iter()
        .fold(HashMap::new, |mut acc: HashMap<K, f64>, item| {
            *acc.entry(key_fn(item)).or_insert(0.0) += val_fn(item);
            acc
        })
        .reduce(HashMap::new, |mut a, b| {
            for (k, v) in b {
                *a.entry(k).or_insert(0.0) += v;
            }
            a
        })
}

/// Parallel filter + transform in one pass.
pub fn par_filter_transform<T, U, F>(data: &[T], f: F) -> Vec<U>
where
    T: Sync,
    U: Send,
    F: Fn(&T) -> Option<U> + Sync + Send,
{
    data.par_iter().filter_map(f).collect()
}

/// Parallel batch processing: split data into chunks, process each chunk.
pub fn par_batch_process<T, U, F>(data: Vec<T>, batch_size: usize, f: F) -> Vec<U>
where
    T: Send,
    U: Send,
    F: Fn(Vec<T>) -> Vec<U> + Sync + Send,
{
    let mut chunks: Vec<Vec<T>> = Vec::new();
    let mut iter = data.into_iter();
    loop {
        let batch: Vec<T> = iter.by_ref().take(batch_size).collect();
        if batch.is_empty() {
            break;
        }
        chunks.push(batch);
    }
    chunks.into_par_iter().flat_map(f).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallel_map_reduce_sum() {
        let data: Vec<i64> = (1..=1000).collect();
        let sum = par_map_reduce(&data, 0i64, |&x| x, |a, b| a + b);
        assert_eq!(sum, 500_500);
    }

    #[test]
    fn parallel_group_by() {
        let data: Vec<(String, f64)> = vec![
            ("a".into(), 1.0),
            ("b".into(), 2.0),
            ("a".into(), 3.0),
            ("b".into(), 4.0),
        ];
        let grouped = par_group_sum(&data, |item| item.0.clone(), |item| item.1);
        assert_eq!(grouped["a"], 4.0);
        assert_eq!(grouped["b"], 6.0);
    }

    #[test]
    fn parallel_batch() {
        let data: Vec<i32> = (1..=10).collect();
        let result = par_batch_process(data, 3, |batch| batch.into_iter().map(|x| x * 2).collect());
        let mut sorted = result;
        sorted.sort();
        assert_eq!(sorted, vec![2, 4, 6, 8, 10, 12, 14, 16, 18, 20]);
    }
}
