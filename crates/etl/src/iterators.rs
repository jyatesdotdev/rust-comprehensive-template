//! Zero-cost iterator chains for data transformation and aggregation.
//!
//! Demonstrates Rust's iterator combinators for ETL-style processing
//! without heap allocation in the pipeline itself.

use std::collections::HashMap;
use std::hash::Hash;

/// A CSV-like record for demonstration.
#[derive(Debug, Clone)]
pub struct Record {
    /// The column values for this record.
    pub fields: Vec<String>,
}

impl Record {
    /// Create a record from string slices.
    pub fn new(fields: Vec<&str>) -> Self {
        Self {
            fields: fields.into_iter().map(String::from).collect(),
        }
    }

    /// Get the field at `idx`, or `None` if out of bounds.
    pub fn get(&self, idx: usize) -> Option<&str> {
        self.fields.get(idx).map(|s| s.as_str())
    }
}

/// Parse raw CSV lines into Records, skipping malformed lines.
pub fn parse_csv(lines: &[&str], expected_cols: usize) -> Vec<Record> {
    lines
        .iter()
        .map(|line| Record {
            fields: line.split(',').map(|s| s.trim().to_owned()).collect(),
        })
        .filter(|r| r.fields.len() == expected_cols)
        .collect()
}

/// Group-by aggregation using iterator fold.
pub fn group_sum<I, K, F, G>(items: I, key_fn: F, val_fn: G) -> HashMap<K, f64>
where
    I: IntoIterator,
    K: Eq + Hash,
    F: Fn(&I::Item) -> K,
    G: Fn(&I::Item) -> f64,
{
    items.into_iter().fold(HashMap::new(), |mut acc, item| {
        *acc.entry(key_fn(&item)).or_insert(0.0) += val_fn(&item);
        acc
    })
}

/// Running average using `scan` — demonstrates stateful iterator processing.
pub fn running_average(data: &[f64]) -> Vec<f64> {
    data.iter()
        .scan((0.0_f64, 0_usize), |(sum, count), &x| {
            *sum += x;
            *count += 1;
            Some(*sum / *count as f64)
        })
        .collect()
}

/// Flatten nested data and transform — demonstrates `flat_map`.
pub fn flatten_transform<T, U, I, F>(nested: I, f: F) -> Vec<U>
where
    I: IntoIterator,
    I::Item: IntoIterator<Item = T>,
    F: Fn(T) -> U,
{
    nested
        .into_iter()
        .flat_map(|inner| inner.into_iter())
        .map(f)
        .collect()
}

/// Top-N selection using partial sort via `select_nth_unstable`.
///
/// If `n >= data.len()`, the whole slice is sorted and returned.
/// Uses `f64::total_cmp` so NaN inputs cannot panic (positive NaN
/// compares greater than every number, so it ranks first if present).
pub fn top_n(data: &mut [f64], n: usize) -> &[f64] {
    if n >= data.len() {
        data.sort_unstable_by(|a, b| b.total_cmp(a));
        return data;
    }
    data.select_nth_unstable_by(n, |a, b| b.total_cmp(a));
    let slice = &mut data[..n];
    slice.sort_unstable_by(|a, b| b.total_cmp(a));
    slice
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_parsing_filters_bad_rows() {
        let lines = vec!["a,b,c", "d,e", "f,g,h"];
        let records = parse_csv(&lines, 3);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].get(1), Some("b"));
    }

    #[test]
    fn group_by_sum() {
        let data = vec![("a", 1.0), ("b", 2.0), ("a", 3.0)];
        let grouped = group_sum(data, |item| item.0, |item| item.1);
        assert_eq!(grouped["a"], 4.0);
        assert_eq!(grouped["b"], 2.0);
    }

    #[test]
    fn running_avg() {
        let avg = running_average(&[2.0, 4.0, 6.0]);
        assert_eq!(avg, vec![2.0, 3.0, 4.0]);
    }

    #[test]
    fn flatten_and_transform() {
        let nested = vec![vec![1, 2], vec![3]];
        let result = flatten_transform(nested, |x| x * 10);
        assert_eq!(result, vec![10, 20, 30]);
    }

    #[test]
    fn top_n_selection() {
        let mut data = vec![3.0, 1.0, 4.0, 1.0, 5.0, 9.0];
        let top = top_n(&mut data, 3);
        assert_eq!(top, &[9.0, 5.0, 4.0]);
    }

    #[test]
    fn top_n_larger_than_input_returns_all_sorted() {
        let mut data = vec![3.0, 1.0, 2.0];
        let top = top_n(&mut data, 10);
        assert_eq!(top, &[3.0, 2.0, 1.0]);
    }

    #[test]
    fn top_n_tolerates_nan() {
        // Before switching to `total_cmp` this panicked; now NaN ranks first
        // (positive NaN is the maximum in the IEEE total order).
        let mut data = vec![f64::NAN, 5.0, 1.0, 3.0];
        let top = top_n(&mut data, 2);
        assert!(top[0].is_nan());
        assert_eq!(top[1], 5.0);
    }
}
