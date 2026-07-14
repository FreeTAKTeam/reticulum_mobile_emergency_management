#[test]
fn app_state_initialization_returns_io_error_when_primary_and_fallback_fail() {
    let root = std::env::temp_dir().join(format!(
        "rem-storage-failure-{}-{}",
        std::process::id(),
        now_ms()
    ));
    std::fs::create_dir_all(&root).expect("create storage failure root");
    let primary = root.join("primary-is-a-file");
    let fallback = root.join("fallback-is-a-file");
    std::fs::write(&primary, b"not a directory").expect("write primary blocker");
    std::fs::write(&fallback, b"not a directory").expect("write fallback blocker");

    let result = create_app_state_store_with_fallback(
        Some(primary.to_string_lossy().as_ref()),
        fallback.to_string_lossy().as_ref(),
    );

    assert!(matches!(result, Err(NodeError::IoError {})));
    std::fs::remove_dir_all(root).expect("remove storage failure root");
}

#[test]
fn node_capability_check_accepts_msgpack_hex_app_data() {
    let payload = MsgPackValue::Array(vec![
        MsgPackValue::from("Msgpack Peer"),
        MsgPackValue::Map(vec![(
            MsgPackValue::from("caps"),
            MsgPackValue::Array(vec![
                MsgPackValue::from("R3AKT"),
                MsgPackValue::from("EMergencyMessages"),
                MsgPackValue::from("Telemetry"),
            ]),
        )]),
    ]);
    let app_data = hex::encode(rmp_serde::to_vec(&payload).expect("msgpack"));

    assert!(has_capability_token(Some(app_data.as_str()), "telemetry"));
    assert!(peer_supports_mission_traffic(&PeerRecord {
        destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        identity_hex: None,
        lxmf_destination_hex: Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string()),
        display_name: None,
        app_data: Some(app_data),
        state: crate::types::PeerState::Disconnected {},
        saved: false,
        stale: false,
        active_link: false,
        last_resolution_error: None,
        last_resolution_attempt_at_ms: None,
        last_seen_at_ms: 1,
        announce_last_seen_at_ms: Some(1),
        lxmf_last_seen_at_ms: Some(1),
        hub_derived: false,
    }));
}

struct TcpRelayHandle {
    addr: SocketAddr,
    shutdown: Arc<Notify>,
    task: tokio::task::JoinHandle<()>,
}

impl TcpRelayHandle {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind relay listener");
        let addr = listener.local_addr().expect("relay local addr");
        let shutdown = Arc::new(Notify::new());
        let clients: Arc<AsyncMutex<HashMap<usize, mpsc::UnboundedSender<Vec<u8>>>>> =
            Arc::new(AsyncMutex::new(HashMap::new()));
        let next_client_id = Arc::new(AtomicUsize::new(1));

        let task = {
            let shutdown = shutdown.clone();
            let clients = clients.clone();
            let next_client_id = next_client_id.clone();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = shutdown.notified() => break,
                        accepted = listener.accept() => {
                            let Ok((stream, _peer)) = accepted else {
                                break;
                            };
                            let client_id = next_client_id.fetch_add(1, AtomicOrdering::Relaxed);
                            let (mut read_half, mut write_half) = stream.into_split();
                            let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
                            clients.lock().await.insert(client_id, tx);

                            let writer_clients = clients.clone();
                            tokio::spawn(async move {
                                while let Some(chunk) = rx.recv().await {
                                    if write_half.write_all(chunk.as_slice()).await.is_err() {
                                        break;
                                    }
                                }
                                writer_clients.lock().await.remove(&client_id);
                            });

                            let reader_clients = clients.clone();
                            tokio::spawn(async move {
                                let mut buf = vec![0u8; 4096];
                                loop {
                                    let read = match read_half.read(&mut buf).await {
                                        Ok(0) => break,
                                        Ok(n) => n,
                                        Err(_) => break,
                                    };
                                    let chunk = buf[..read].to_vec();
                                    let mut guard = reader_clients.lock().await;
                                    let mut dead_clients = Vec::new();
                                    for (peer_id, sender) in guard.iter() {
                                        if *peer_id == client_id {
                                            continue;
                                        }
                                        if sender.send(chunk.clone()).is_err() {
                                            dead_clients.push(*peer_id);
                                        }
                                    }
                                    for peer_id in dead_clients {
                                        guard.remove(&peer_id);
                                    }
                                }
                                reader_clients.lock().await.remove(&client_id);
                            });
                        }
                    }
                }
            })
        };

        Self {
            addr,
            shutdown,
            task,
        }
    }

    fn address(&self) -> String {
        self.addr.to_string()
    }

    async fn shutdown(self) {
        self.shutdown.notify_waiters();
        let _ = self.task.await;
    }
}

fn test_lock() -> &'static AsyncMutex<()> {
    TEST_LOCK.get_or_init(|| AsyncMutex::new(()))
}

fn wait_until_running(node: &Node) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if node.get_status().running {
            return;
        }
        if Instant::now() >= deadline {
            panic!("node did not report running in time");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

struct CurrentDirGuard {
    previous: PathBuf,
}

impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.previous);
    }
}

fn isolate_current_dir(name: &str) -> CurrentDirGuard {
    let previous = std::env::current_dir().expect("capture current dir");
    let dir = prepare_storage_dir(name);
    std::env::set_current_dir(&dir).expect("set current dir");
    CurrentDirGuard { previous }
}

fn unique_test_dir(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "reticulum_mobile_e2e_{}_{}_{}",
        name,
        std::process::id(),
        stamp
    ))
}

fn prepare_storage_dir(name: &str) -> PathBuf {
    let dir = unique_test_dir(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create storage dir");
    dir
}

fn build_config(name: &str, storage_dir: &Path, relay_addr: &str) -> NodeConfig {
    NodeConfig {
        name: name.to_string(),
        storage_dir: Some(storage_dir.to_string_lossy().to_string()),
        tcp_clients: vec![relay_addr.to_string()],
        broadcast: true,
        transport_node_enabled: true,
        announce_interval_seconds: 1,
        stale_after_minutes: 30,
        announce_capabilities: "R3AKT,EMergencyMessages,Telemetry".to_string(),
        hub_mode: HubMode::Autonomous {},
        hub_identity_hash: None,
        hub_api_base_url: None,
        hub_api_key: None,
        hub_refresh_interval_seconds: 0,
        rnode: crate::types::RnodeSettingsRecord::default(),
    }
}

fn wait_for_event<F>(
    subscription: &Arc<EventSubscription>,
    timeout: Duration,
    mut predicate: F,
) -> Option<NodeEvent>
where
    F: FnMut(&NodeEvent) -> bool,
{
    let deadline = Instant::now() + timeout;
    loop {
        if Instant::now() >= deadline {
            return None;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timeout_ms = remaining.as_millis().min(u32::MAX as u128).max(1) as u32;
        if let Some(event) = subscription.next(timeout_ms.min(250)) {
            if predicate(&event) {
                return Some(event);
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn send_emergency_message_is_received_as_mission_packet() {
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("emergency").await;

    let node_a_status = node_a.get_status();
    let node_b_status = node_b.get_status();
    let body = "emergency: request medevac";
    let fields = mission_command_fields(
        "cmd-eam-123",
        "corr-eam-123",
        "mission.registry.eam.upsert",
        vec![
            ("eam_uid", MsgPackValue::from("eam-123")),
            ("team_member_uid", MsgPackValue::from("member-1")),
            ("team_uid", MsgPackValue::from("team-1")),
            ("mission_uid", MsgPackValue::from("mission-1")),
        ],
    );
    let subscription = node_b.subscribe_events();
    node_a
        .send_bytes(
            node_b_status.lxmf_destination_hex.clone(),
            body.as_bytes().to_vec(),
            Some(fields.clone()),
            SendMode::Auto {},
        )
        .expect("send emergency packet");

    let event = wait_for_event(&subscription, TEST_TIMEOUT, |event| {
        matches!(event, NodeEvent::PacketReceived { bytes, .. } if bytes.as_slice() == body.as_bytes())
    })
    .expect("node b received emergency packet");

    assert_packet_received(
        event,
        &node_a_status.lxmf_destination_hex,
        body,
        Some(fields.as_slice()),
    );

    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}
