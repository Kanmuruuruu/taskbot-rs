use std::sync::{Arc, Mutex};
use std::time::Duration;
use taskbot_rs::{Scheduler, TaskBuilder};
use tokio;

#[tokio::test]
async fn test_task_success_on_first_try() {
    let success_flag = Arc::new(Mutex::new(false));
    let success_flag_clone = success_flag.clone();

    let task = TaskBuilder::new("success_task", || tokio::spawn(async { Ok::<(), ()>(()) }))
        .on_success(move || {
            let mut flag = success_flag_clone.lock().unwrap();
            *flag = true;
        })
        .on_failure(|| panic!("Should not fail"))
        .build();

    let mut scheduler = Scheduler::new();
    scheduler.add(task);

    tokio::time::timeout(Duration::from_millis(200), scheduler.run_once())
        .await
        .unwrap_or(());

    assert!(
        *success_flag.lock().unwrap(),
        "on_success should have been called"
    );
}

#[tokio::test]
async fn test_task_retry_until_success() {
    let counter = Arc::new(Mutex::new(0));
    let success_flag = Arc::new(Mutex::new(false));

    let counter_clone = counter.clone();
    let success_clone = success_flag.clone();

    let task = TaskBuilder::new("retry_task", move || {
        let counter_inner = counter_clone.clone();
        tokio::spawn(async move {
            let mut num = counter_inner.lock().unwrap();
            *num += 1;
            if *num < 3 {
                Err::<(), i32>(1)
            } else {
                Ok::<(), i32>(())
            }
        })
    })
    .max_retries(5)
    .retry_base_delay(Duration::from_millis(10))
    .retry_errors(vec![1])
    .on_success(move || {
        let mut flag = success_clone.lock().unwrap();
        *flag = true;
    })
    .on_failure(|| panic!("Should not fail"))
    .build();

    let mut scheduler = Scheduler::new();
    scheduler.add(task);

    tokio::time::timeout(Duration::from_secs(2), scheduler.run_once())
        .await
        .unwrap_or(());

    let final_count = *counter.lock().unwrap();
    assert!(
        final_count >= 3,
        "Task should have retried at least 3 times"
    );
    assert!(
        *success_flag.lock().unwrap(),
        "on_success should have been called"
    );
}

#[tokio::test]
async fn test_task_failure_after_retries() {
    let failure_flag = Arc::new(Mutex::new(false));
    let failure_flag_clone = failure_flag.clone();

    let task = TaskBuilder::new("fail_task", || tokio::spawn(async { Err::<(), i32>(1) }))
        .max_retries(2)
        .retry_base_delay(Duration::from_millis(10))
        .retry_errors(vec![1])
        .on_success(|| panic!("Should not succeed"))
        .on_failure(move || {
            let mut flag = failure_flag_clone.lock().unwrap();
            *flag = true;
        })
        .build();

    let mut scheduler = Scheduler::new();
    scheduler.add(task);

    tokio::time::timeout(Duration::from_secs(1), scheduler.run_once())
        .await
        .unwrap_or(());

    assert!(
        *failure_flag.lock().unwrap(),
        "on_failure should have been called"
    );
}

#[tokio::test]
async fn test_multiple_tasks_simultaneously() {
    let flags = Arc::new(Mutex::new(vec![false, false, false]));
    let flags_clone1 = flags.clone();
    let flags_clone2 = flags.clone();
    let flags_clone3 = flags.clone();

    let task1 = TaskBuilder::new("task1", || tokio::spawn(async { Ok::<(), ()>(()) }))
        .on_success(move || {
            let mut f = flags_clone1.lock().unwrap();
            f[0] = true;
        })
        .build();

    let task2 = TaskBuilder::new("task2", || tokio::spawn(async { Ok::<(), ()>(()) }))
        .on_success(move || {
            let mut f = flags_clone2.lock().unwrap();
            f[1] = true;
        })
        .build();

    let task3 = TaskBuilder::new("task3", || tokio::spawn(async { Ok::<(), ()>(()) }))
        .on_success(move || {
            let mut f = flags_clone3.lock().unwrap();
            f[2] = true;
        })
        .build();

    let mut scheduler = Scheduler::new();
    scheduler.add(task1);
    scheduler.add(task2);
    scheduler.add(task3);

    tokio::time::timeout(Duration::from_secs(1), scheduler.run_once())
        .await
        .unwrap_or(());

    let final_flags = flags.lock().unwrap();
    assert!(
        final_flags[0] && final_flags[1] && final_flags[2],
        "All tasks should succeed"
    );
}

#[tokio::test]
async fn test_task_with_timeout() {
    let flag = Arc::new(Mutex::new(false));
    let flag_clone = flag.clone();

    let task = TaskBuilder::new("timeout_task", || {
        tokio::spawn(async {
            tokio::time::sleep(Duration::from_millis(200)).await;
            Ok::<(), ()>(())
        })
    })
    .timeout(Duration::from_millis(50))
    .on_success(|| panic!("Should timeout"))
    .on_failure(move || {
        let mut f = flag_clone.lock().unwrap();
        *f = true;
    })
    .build();

    let mut scheduler = Scheduler::new();
    scheduler.add(task);

    tokio::time::timeout(Duration::from_secs(1), scheduler.run_once())
        .await
        .unwrap_or(());

    assert!(
        *flag.lock().unwrap(),
        "on_failure should have been called due to timeout"
    );
}
