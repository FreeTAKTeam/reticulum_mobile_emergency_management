const CONTROL_COMMAND_TASK_CONCURRENCY: usize = 1;
const CONTROL_COMMAND_QUEUE_CAPACITY: usize = 256;
const WORK_COMMAND_TASK_CONCURRENCY: usize = 16;
const WORK_COMMAND_QUEUE_CAPACITY: usize = 256;
const LOCAL_COMMAND_TASK_CONCURRENCY: usize = 8;
const LOCAL_COMMAND_QUEUE_CAPACITY: usize = 256;
const PRIORITY_COMMAND_TASK_CONCURRENCY: usize = 4;
const PRIORITY_COMMAND_QUEUE_CAPACITY: usize = 1_024;
const PRIORITY_CONTROL_COMMAND_QUEUE_CAPACITY: usize = 256;

type RuntimeCommandJob = std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuntimeCommandLane {
    Normal,
    Priority,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuntimeCommandClass {
    Control,
    Work,
    Local,
}

#[derive(Clone, Copy)]
struct RuntimeCommandExecutorConfig {
    control: (usize, usize),
    work: (usize, usize),
    local: (usize, usize),
    priority: (usize, usize),
    priority_control: (usize, usize),
}

#[derive(Clone)]
struct RuntimeCommandExecutor {
    control: mpsc::Sender<RuntimeCommandJob>,
    work: mpsc::Sender<RuntimeCommandJob>,
    local: mpsc::Sender<RuntimeCommandJob>,
    priority: mpsc::Sender<RuntimeCommandJob>,
    priority_control: mpsc::Sender<RuntimeCommandJob>,
}

impl RuntimeCommandExecutor {
    fn new() -> Self {
        Self::with_config(RuntimeCommandExecutorConfig {
            control: (
                CONTROL_COMMAND_TASK_CONCURRENCY,
                CONTROL_COMMAND_QUEUE_CAPACITY,
            ),
            work: (WORK_COMMAND_TASK_CONCURRENCY, WORK_COMMAND_QUEUE_CAPACITY),
            local: (LOCAL_COMMAND_TASK_CONCURRENCY, LOCAL_COMMAND_QUEUE_CAPACITY),
            priority: (
                PRIORITY_COMMAND_TASK_CONCURRENCY,
                PRIORITY_COMMAND_QUEUE_CAPACITY,
            ),
            priority_control: (1, PRIORITY_CONTROL_COMMAND_QUEUE_CAPACITY),
        })
    }

    fn with_config(config: RuntimeCommandExecutorConfig) -> Self {
        Self {
            control: spawn_runtime_command_workers(config.control.0, config.control.1),
            work: spawn_runtime_command_workers(config.work.0, config.work.1),
            local: spawn_runtime_command_workers(config.local.0, config.local.1),
            priority: spawn_runtime_command_workers(config.priority.0, config.priority.1),
            priority_control: spawn_runtime_command_workers(
                config.priority_control.0,
                config.priority_control.1,
            ),
        }
    }

    fn sender(
        &self,
        lane: RuntimeCommandLane,
        class: RuntimeCommandClass,
    ) -> &mpsc::Sender<RuntimeCommandJob> {
        match (lane, class) {
            (RuntimeCommandLane::Priority, RuntimeCommandClass::Control) => {
                &self.priority_control
            }
            (RuntimeCommandLane::Priority, _) => &self.priority,
            (RuntimeCommandLane::Normal, class) => match class {
                RuntimeCommandClass::Control => &self.control,
                RuntimeCommandClass::Work => &self.work,
                RuntimeCommandClass::Local => &self.local,
            },
        }
    }

    fn spawn<T, F>(
        &self,
        lane: RuntimeCommandLane,
        class: RuntimeCommandClass,
        resp: cb::Sender<Result<T, NodeError>>,
        task: F,
    )
    where
        T: Send + 'static,
        F: std::future::Future<Output = Result<T, NodeError>> + Send + 'static,
    {
        let Ok(queue_slot) = self.sender(lane, class).try_reserve() else {
            warn!("[runtime][command] rejected lane={lane:?} class={class:?} reason=saturated");
            let _ = resp.send(Err(NodeError::Timeout {}));
            return;
        };
        queue_slot.send(Box::pin(async move {
            let _ = resp.send(task.await);
        }));
    }

    fn spawn_detached<F>(&self, lane: RuntimeCommandLane, class: RuntimeCommandClass, task: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let Ok(queue_slot) = self.sender(lane, class).try_reserve() else {
            warn!("[runtime][command] dropped lane={lane:?} class={class:?} reason=saturated");
            return;
        };
        queue_slot.send(Box::pin(async move {
            task.await;
        }));
    }
}

fn spawn_runtime_command_workers(
    concurrency: usize,
    capacity: usize,
) -> mpsc::Sender<RuntimeCommandJob> {
    assert!(concurrency > 0, "command concurrency must be non-zero");
    assert!(capacity > 0, "command queue capacity must be non-zero");
    let (tx, mut rx) = mpsc::channel::<RuntimeCommandJob>(capacity);
    let permits = Arc::new(Semaphore::new(concurrency));
    tokio::spawn(async move {
        while let Some(job) = rx.recv().await {
            let Ok(permit) = permits.clone().acquire_owned().await else {
                break;
            };
            tokio::spawn(async move {
                let _permit = permit;
                job.await;
            });
        }
    });
    tx
}
