use futures::Future;
use rand::Rng;
use std::pin::Pin;
use std::time::Duration;
use tokio::time::sleep;

pub async fn retry_async<F, T, E>(attempts: u32, base_delay: Duration, mut f: F) -> Result<T, E>
where
    F: FnMut() -> Pin<Box<dyn Future<Output = Result<T, E>> + Send>> + Send,
{
    for i in 0..attempts {
        let res = f().await;
        if res.is_ok() {
            return res;
        }

        if i + 1 < attempts {
            let delay = backoff_with_jitter(base_delay, i + 1);
            sleep(delay).await;
        } else {
            return res;
        }
    }
    unreachable!()
}

fn backoff_with_jitter(base: Duration, attempt: u32) -> Duration {
    let exp = base * 2u32.pow(attempt - 1);
    let jitter: u64 = rand::thread_rng().gen_range(0..base.as_millis() as u64);
    Duration::from_millis(exp.as_millis() as u64 + jitter)
}
