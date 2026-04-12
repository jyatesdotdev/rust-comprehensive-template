//! Tokio async patterns: task spawning, channels, select, concurrent futures.

use tokio::sync::{mpsc, oneshot};

/// Fan-out/fan-in: spawn `n` tasks, each producing a result, collect all results.
pub async fn fan_out<F, Fut, T>(n: usize, task_fn: F) -> Vec<T>
where
    F: Fn(usize) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let task_fn = std::sync::Arc::new(task_fn);
    let mut handles = Vec::with_capacity(n);
    for i in 0..n {
        let f = task_fn.clone();
        handles.push(tokio::spawn(async move { f(i).await }));
    }
    let mut results = Vec::with_capacity(n);
    for h in handles {
        results.push(h.await.expect("task panicked"));
    }
    results
}

/// Producer-consumer via bounded mpsc channel.
pub async fn producer_consumer<T, P, C, R>(
    buffer: usize,
    produce: P,
    consume: C,
) -> R
where
    T: Send + 'static,
    P: FnOnce(mpsc::Sender<T>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
        + Send
        + 'static,
    C: FnOnce(mpsc::Receiver<T>) -> std::pin::Pin<Box<dyn std::future::Future<Output = R> + Send>>
        + Send
        + 'static,
    R: Send + 'static,
{
    let (tx, rx) = mpsc::channel(buffer);
    let producer = tokio::spawn(produce(tx));
    let consumer = tokio::spawn(consume(rx));
    let _ = producer.await;
    consumer.await.expect("consumer panicked")
}

/// Request-response pattern using oneshot channels.
pub async fn request_response<Req, Resp, F, Fut>(request: Req, handler: F) -> Resp
where
    Req: Send + 'static,
    Resp: Send + 'static,
    F: FnOnce(Req) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Resp> + Send + 'static,
{
    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move {
        let resp = handler(request).await;
        let _ = tx.send(resp);
    });
    rx.await.expect("handler dropped without responding")
}

/// Run multiple futures concurrently, return first to complete (select pattern).
pub async fn race<A, B>(
    a: impl std::future::Future<Output = A> + Send,
    b: impl std::future::Future<Output = B> + Send,
) -> Either<A, B> {
    tokio::select! {
        val = a => Either::Left(val),
        val = b => Either::Right(val),
    }
}

/// Result of [`race`]: whichever future completed first.
#[derive(Debug, PartialEq)]
pub enum Either<A, B> {
    /// The first future completed first.
    Left(A),
    /// The second future completed first.
    Right(B),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fan_out() {
        let results = fan_out(5, |i| async move { i * 2 }).await;
        assert_eq!(results, vec![0, 2, 4, 6, 8]);
    }

    #[tokio::test]
    async fn test_producer_consumer() {
        let sum: i32 = producer_consumer(
            16,
            |tx| {
                Box::pin(async move {
                    for i in 0..5 {
                        tx.send(i).await.unwrap();
                    }
                })
            },
            |mut rx| {
                Box::pin(async move {
                    let mut total = 0;
                    while let Some(v) = rx.recv().await {
                        total += v;
                    }
                    total
                })
            },
        )
        .await;
        assert_eq!(sum, 10);
    }

    #[tokio::test]
    async fn test_request_response() {
        let resp: String =
            request_response(42, |n| async move { format!("answer: {n}") }).await;
        assert_eq!(resp, "answer: 42");
    }

    #[tokio::test]
    async fn test_race() {
        let result = race(async { 1 }, std::future::pending::<i32>()).await;
        assert_eq!(result, Either::Left(1));
    }
}
