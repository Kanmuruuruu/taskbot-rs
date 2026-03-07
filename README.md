# taskbot-rs

Scheduler Rust to execute repetitives tasks with **retry**, **timeout**, **logs**, et **callbacks**.

---

## Installation

Add the dependency in your `Cargo.toml` :

```toml
[dependencies]
taskbot-rs = "0.1.0"
```

Example

```rs
use std::sync::{Arc, Mutex};
use std::time::Duration;
use taskbot_rs::{TaskBuilder, Scheduler};
use tokio;

#[tokio::main]
async fn main() {
    let success_flag = Arc::new(Mutex::new(false));
    let success_clone = success_flag.clone();

    let task = TaskBuilder::new("example_task", || {
        tokio::spawn(async {
            println!("Running task...");
            Ok::<(), ()>(())
        })
    })
    .interval(Duration::from_secs(2))
    .max_retries(3)
    .retry_base_delay(Duration::from_millis(100))
    .timeout(Duration::from_secs(5))
    .on_success(move || {
        let mut flag = success_clone.lock().unwrap();
        *flag = true;
        println!("Task succeeded!");
    })
    .on_failure(|| {
        println!("Task failed after retries.");
    })
    .build();

    let mut scheduler = Scheduler::new();
    scheduler.add(task);

    scheduler.run_once().await;

}
```
