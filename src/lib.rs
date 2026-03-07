mod retrix;

use retrix::retry_async;
use std::sync::Arc;
use std::time::Duration;
use tokio::task;

pub struct Task<E> {
    pub name: String,
    pub interval: Duration,
    pub max_retries: u32,
    pub retry_base_delay: Duration,
    pub timeout: Duration,
    pub action: Arc<dyn Fn() -> task::JoinHandle<Result<(), E>> + Send + Sync>,
    pub retry_errors: Vec<E>,
    pub logger: Option<Arc<dyn Fn(&str) + Send + Sync>>,
    pub on_success: Option<Arc<dyn Fn() + Send + Sync>>,
    pub on_failure: Option<Arc<dyn Fn() + Send + Sync>>,
}

pub struct TaskBuilder<E> {
    name: String,
    action: Arc<dyn Fn() -> task::JoinHandle<Result<(), E>> + Send + Sync>,
    interval: Duration,
    max_retries: u32,
    retry_base_delay: Duration,
    timeout: Duration,
    retry_errors: Vec<E>,
    logger: Option<Arc<dyn Fn(&str) + Send + Sync>>,
    on_success: Option<Arc<dyn Fn() + Send + Sync>>,
    on_failure: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl<E: Clone + PartialEq + Send + 'static> TaskBuilder<E> {
    pub fn new<F>(name: &str, action: F) -> Self
    where
        F: Fn() -> task::JoinHandle<Result<(), E>> + Send + Sync + 'static,
    {
        Self {
            name: name.to_string(),
            action: Arc::new(action),
            interval: Duration::from_secs(5),
            max_retries: 3,
            retry_base_delay: Duration::from_millis(100),
            timeout: Duration::from_secs(10),
            retry_errors: vec![],
            logger: None,
            on_success: None,
            on_failure: None,
        }
    }

    pub fn interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    pub fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    pub fn retry_base_delay(mut self, delay: Duration) -> Self {
        self.retry_base_delay = delay;
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn retry_errors(mut self, errors: Vec<E>) -> Self {
        self.retry_errors = errors;
        self
    }

    pub fn logger<FN>(mut self, logger: FN) -> Self
    where
        FN: Fn(&str) + Send + Sync + 'static,
    {
        self.logger = Some(Arc::new(logger));
        self
    }

    pub fn on_success<FN>(mut self, callback: FN) -> Self
    where
        FN: Fn() + Send + Sync + 'static,
    {
        self.on_success = Some(Arc::new(callback));
        self
    }

    pub fn on_failure<FN>(mut self, callback: FN) -> Self
    where
        FN: Fn() + Send + Sync + 'static,
    {
        self.on_failure = Some(Arc::new(callback));
        self
    }

    pub fn build(self) -> Task<E> {
        Task {
            name: self.name,
            action: self.action,
            interval: self.interval,
            max_retries: self.max_retries,
            retry_base_delay: self.retry_base_delay,
            timeout: self.timeout,
            retry_errors: self.retry_errors,
            logger: self.logger,
            on_success: self.on_success,
            on_failure: self.on_failure,
        }
    }
}

pub struct Scheduler<E> {
    tasks: Vec<Task<E>>,
}

impl<E: Clone + PartialEq + Send + Default + 'static> Scheduler<E> {
    pub fn new() -> Self {
        Self { tasks: vec![] }
    }

    pub fn add(&mut self, task: Task<E>) {
        self.tasks.push(task);
    }

    pub async fn run_once(&self) {
        for task in &self.tasks {
            let t = task.clone_task();
            let log = |msg: &str| {
                if let Some(logger) = &t.logger {
                    (logger)(msg);
                }
            };

            let action_closure = t.action.clone();
            let result = retrix::retry_async(t.max_retries, t.retry_base_delay, || {
                let handle = (action_closure)();
                let timeout = t.timeout;
                Box::pin(async move {
                    let res = tokio::time::timeout(timeout, handle).await;
                    match res {
                        Ok(join_res) => match join_res {
                            Ok(inner_res) => inner_res,
                            Err(join_err) => {
                                eprintln!("Task spawn failed: {}", join_err);
                                Err(E::default())
                            }
                        },
                        Err(_) => {
                            eprintln!("Task timed out after {:?}", timeout);
                            Err(E::default())
                        }
                    }
                })
            })
            .await;

            match result {
                Ok(_) => {
                    if let Some(cb) = &t.on_success {
                        (cb)();
                    }
                }
                Err(_) => {
                    if let Some(cb) = &t.on_failure {
                        (cb)();
                    }
                }
            }

            log(&format!("Task {} executed/retried", t.name));
        }
    }

    pub async fn run(&self) {
        for task in &self.tasks {
            let t = task.clone_task();
            tokio::spawn(async move {
                loop {
                    let log = |msg: &str| {
                        if let Some(logger) = &t.logger {
                            (logger)(msg);
                        }
                    };

                    let action_closure = t.action.clone();

                    let result = retry_async(t.max_retries, t.retry_base_delay, || {
                        let handle = (action_closure)();
                        let timeout = t.timeout;
                        Box::pin(async move {
                            let res = tokio::time::timeout(timeout, handle).await;
                            match res {
                                Ok(join_res) => match join_res {
                                    Ok(inner_res) => inner_res,
                                    Err(join_err) => {
                                        eprintln!("Task spawn failed: {}", join_err);
                                        Ok(())
                                    }
                                },
                                Err(_) => {
                                    eprintln!("Task timed out after {:?}", timeout);
                                    Ok(())
                                }
                            }
                        })
                    })
                    .await;

                    match result {
                        Ok(_) => {
                            if let Some(cb) = &t.on_success {
                                (cb)();
                            }
                        }
                        Err(_) => {
                            if let Some(cb) = &t.on_failure {
                                (cb)();
                            }
                        }
                    }

                    log(&format!("Task {} executed/retried", t.name));
                    tokio::time::sleep(t.interval).await;
                }
            });
        }
    }
}

impl<E: Clone + PartialEq + Send + 'static> Task<E> {
    fn clone_task(&self) -> Self {
        Self {
            name: self.name.clone(),
            interval: self.interval,
            max_retries: self.max_retries,
            retry_base_delay: self.retry_base_delay,
            timeout: self.timeout,
            action: self.action.clone(),
            retry_errors: self.retry_errors.clone(),
            logger: self.logger.clone(),
            on_success: self.on_success.clone(),
            on_failure: self.on_failure.clone(),
        }
    }
}
