use std::time::Duration;
use taskbot_rs::{Scheduler, TaskBuilder};
use tokio;

#[tokio::main]
async fn main() {
    let task = TaskBuilder::new("demo_task", || {
        tokio::spawn(async {
            println!("Tentative de tâche...");
            Err::<(), i32>(1)
        })
    })
    .max_retries(3)
    .retry_base_delay(Duration::from_millis(50))
    .on_success(|| println!("🎉 Task succeeded!"))
    .on_failure(|| println!("💥 Task failed after all retries"))
    .build();

    let mut scheduler = Scheduler::new();
    scheduler.add(task);
    scheduler.run().await;
}
