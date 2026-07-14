#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn saturated_normal_commands_do_not_consume_priority_capacity() {
    let executor = RuntimeCommandExecutor::with_config(RuntimeCommandExecutorConfig {
        control: (1, 1),
        work: (1, 1),
        local: (1, 1),
        priority: (1, 1),
        priority_control: (1, 1),
    });
    let release = Arc::new(tokio::sync::Notify::new());
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (first_resp_tx, first_resp_rx) = cb::bounded(1);
    let first_release = release.clone();

    executor.spawn(
        RuntimeCommandLane::Normal,
        RuntimeCommandClass::Work,
        first_resp_tx,
        async move {
        let _ = started_tx.send(());
        first_release.notified().await;
        Ok::<(), NodeError>(())
        },
    );
    started_rx.await.expect("normal command should start");

    let (queued_tx, queued_rx) = cb::bounded(1);
    executor.spawn(
        RuntimeCommandLane::Normal,
        RuntimeCommandClass::Work,
        queued_tx,
        async { Ok::<(), NodeError>(()) },
    );
    let (rejected_tx, rejected_rx) = cb::bounded(1);
    executor.spawn(
        RuntimeCommandLane::Normal,
        RuntimeCommandClass::Work,
        rejected_tx,
        async { Ok::<(), NodeError>(()) },
    );
    let rejected = tokio::task::spawn_blocking(move || {
        rejected_rx.recv_timeout(Duration::from_millis(100))
    })
    .await
    .expect("rejection wait should join")
    .expect("saturated command should receive a response");
    assert!(matches!(rejected, Err(NodeError::Timeout {})));

    let (priority_tx, priority_rx) = cb::bounded(1);
    executor.spawn(
        RuntimeCommandLane::Priority,
        RuntimeCommandClass::Work,
        priority_tx,
        async { Ok::<(), NodeError>(()) },
    );
    let priority = tokio::task::spawn_blocking(move || {
        priority_rx.recv_timeout(Duration::from_millis(100))
    })
    .await
    .expect("priority wait should join")
    .expect("priority command should complete");
    assert!(priority.is_ok());

    release.notify_waiters();
    let first = tokio::task::spawn_blocking(move || {
        first_resp_rx.recv_timeout(Duration::from_millis(100))
    })
    .await
    .expect("normal wait should join")
    .expect("normal command should complete after release");
    assert!(first.is_ok());

    let queued = tokio::task::spawn_blocking(move || {
        queued_rx.recv_timeout(Duration::from_millis(100))
    })
    .await
    .expect("queued wait should join")
    .expect("queued normal command should complete");
    assert!(queued.is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn priority_control_commands_remain_fifo_without_blocking_sos_or_local_work() {
    let executor = RuntimeCommandExecutor::with_config(RuntimeCommandExecutorConfig {
        control: (1, 2),
        work: (1, 1),
        local: (1, 1),
        priority: (1, 1),
        priority_control: (1, 1),
    });
    let release = Arc::new(tokio::sync::Notify::new());
    let order = Arc::new(std::sync::Mutex::new(Vec::new()));
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (first_tx, first_rx) = cb::bounded(1);
    let first_release = release.clone();
    let first_order = order.clone();
    executor.spawn(
        RuntimeCommandLane::Priority,
        RuntimeCommandClass::Control,
        first_tx,
        async move {
            first_order.lock().expect("order lock poisoned").push(1);
            let _ = started_tx.send(());
            first_release.notified().await;
            Ok::<(), NodeError>(())
        },
    );
    started_rx.await.expect("first control command should start");

    let (second_tx, second_rx) = cb::bounded(1);
    let second_order = order.clone();
    executor.spawn(
        RuntimeCommandLane::Priority,
        RuntimeCommandClass::Control,
        second_tx,
        async move {
            second_order.lock().expect("order lock poisoned").push(2);
            Ok::<(), NodeError>(())
        },
    );

    let (local_tx, local_rx) = cb::bounded(1);
    executor.spawn(
        RuntimeCommandLane::Normal,
        RuntimeCommandClass::Local,
        local_tx,
        async { Ok::<(), NodeError>(()) },
    );
    let local = tokio::task::spawn_blocking(move || {
        local_rx.recv_timeout(Duration::from_millis(100))
    })
    .await
    .expect("local wait should join")
    .expect("local query should not wait for control work");
    assert!(local.is_ok());
    assert!(matches!(second_rx.try_recv(), Err(cb::TryRecvError::Empty)));

    let (sos_tx, sos_rx) = cb::bounded(1);
    executor.spawn(
        RuntimeCommandLane::Priority,
        RuntimeCommandClass::Work,
        sos_tx,
        async { Ok::<(), NodeError>(()) },
    );
    let sos = tokio::task::spawn_blocking(move || {
        sos_rx.recv_timeout(Duration::from_millis(100))
    })
    .await
    .expect("SOS wait should join")
    .expect("SOS work should not wait for priority control work");
    assert!(sos.is_ok());

    release.notify_waiters();
    for receiver in [first_rx, second_rx] {
        let result = tokio::task::spawn_blocking(move || {
            receiver.recv_timeout(Duration::from_millis(100))
        })
        .await
        .expect("control wait should join")
        .expect("control command should complete");
        assert!(result.is_ok());
    }
    assert_eq!(*order.lock().expect("order lock poisoned"), vec![1, 2]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn priority_queue_accepts_defined_thousand_peer_burst() {
    const PEER_BURST: usize = 1_000;
    let executor = RuntimeCommandExecutor::with_config(RuntimeCommandExecutorConfig {
        control: (1, 1),
        work: (1, 1),
        local: (1, 1),
        priority: (1, PRIORITY_COMMAND_QUEUE_CAPACITY),
        priority_control: (1, 1),
    });
    let release = Arc::new(tokio::sync::Notify::new());
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (first_tx, first_rx) = cb::bounded(1);
    let first_release = release.clone();
    executor.spawn(
        RuntimeCommandLane::Priority,
        RuntimeCommandClass::Work,
        first_tx,
        async move {
        let _ = started_tx.send(());
        first_release.notified().await;
        Ok::<(), NodeError>(())
        },
    );
    started_rx.await.expect("first priority command should start");

    let mut receivers = Vec::with_capacity(PEER_BURST);
    receivers.push(first_rx);
    for _ in 1..PEER_BURST {
        let (tx, rx) = cb::bounded(1);
        executor.spawn(
            RuntimeCommandLane::Priority,
            RuntimeCommandClass::Work,
            tx,
            async { Ok::<(), NodeError>(()) },
        );
        assert!(matches!(rx.try_recv(), Err(cb::TryRecvError::Empty)));
        receivers.push(rx);
    }

    release.notify_waiters();
    tokio::task::spawn_blocking(move || {
        for rx in receivers {
            assert!(rx
                .recv_timeout(Duration::from_secs(2))
                .expect("priority command should complete")
                .is_ok());
        }
    })
    .await
    .expect("priority completion wait should join");
}
