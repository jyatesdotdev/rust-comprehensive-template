//! Async streaming pipelines with backpressure via tokio channels.
//!
//! Demonstrates multi-stage async ETL where each stage runs as a
//! separate task, connected by bounded channels for backpressure.

use tokio::sync::mpsc;

/// Run a multi-stage async pipeline: source → transform stages → sink.
///
/// Each stage runs concurrently. Bounded channels provide backpressure —
/// fast producers block when slow consumers fall behind.
pub async fn streaming_pipeline<T, U, F>(
    source: Vec<T>,
    transform: F,
    buffer: usize,
) -> Vec<U>
where
    T: Send + 'static,
    U: Send + 'static,
    F: Fn(T) -> Option<U> + Send + 'static,
{
    let (tx, mut rx_mid) = mpsc::channel::<T>(buffer);
    let (tx_out, mut rx_out) = mpsc::channel::<U>(buffer);

    // Producer: feed source items into the pipeline.
    tokio::spawn(async move {
        for item in source {
            if tx.send(item).await.is_err() {
                break;
            }
        }
    });

    // Transform stage: apply function, forward results.
    tokio::spawn(async move {
        while let Some(item) = rx_mid.recv().await {
            if let Some(result) = transform(item) {
                if tx_out.send(result).await.is_err() {
                    break;
                }
            }
        }
    });

    // Sink: collect all results.
    let mut results = Vec::new();
    while let Some(item) = rx_out.recv().await {
        results.push(item);
    }
    results
}

/// Fan-out pipeline: distribute work across `n` parallel transform workers.
pub async fn fan_out_pipeline<T, U, F>(
    source: Vec<T>,
    transform: F,
    workers: usize,
    buffer: usize,
) -> Vec<U>
where
    T: Send + 'static,
    U: Send + 'static,
    F: Fn(T) -> U + Send + Sync + Clone + 'static,
{
    let (tx_in, rx_in) = async_channel::bounded::<T>(buffer);
    let (tx_out, mut rx_out) = mpsc::channel::<U>(buffer * workers);

    // Producer
    tokio::spawn(async move {
        for item in source {
            if tx_in.send(item).await.is_err() {
                break;
            }
        }
    });

    // Spawn N workers that pull from the shared input channel.
    for _ in 0..workers {
        let rx = rx_in.clone();
        let tx = tx_out.clone();
        let f = transform.clone();
        tokio::spawn(async move {
            while let Ok(item) = rx.recv().await {
                let result = f(item);
                if tx.send(result).await.is_err() {
                    break;
                }
            }
        });
    }
    drop(tx_out); // Close when all workers finish.

    let mut results = Vec::new();
    while let Some(item) = rx_out.recv().await {
        results.push(item);
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn streaming_filter_transform() {
        let data: Vec<i32> = (1..=10).collect();
        let result = streaming_pipeline(data, |x| if x % 2 == 0 { Some(x * 10) } else { None }, 4).await;
        assert_eq!(result, vec![20, 40, 60, 80, 100]);
    }

    #[tokio::test]
    async fn fan_out_processes_all() {
        let data: Vec<i32> = (1..=20).collect();
        let mut result = fan_out_pipeline(data, |x| x * 2, 4, 8).await;
        result.sort();
        let expected: Vec<i32> = (1..=20).map(|x| x * 2).collect();
        assert_eq!(result, expected);
    }
}
