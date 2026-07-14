async fn connect_tcp_endpoint(connect_addr: &str) -> Option<TcpStream> {
    match connect_tcp_endpoint_with_error(connect_addr).await {
        Ok(stream) => Some(stream),
        Err(error) => {
            warn!(
                "tcp_client: connect failed endpoint=<{}>: {}",
                connect_addr, error
            );
            None
        }
    }
}

async fn connect_tcp_endpoint_with_error(connect_addr: &str) -> Result<TcpStream, String> {
    let addresses = tokio::time::timeout(TCP_CLIENT_CONNECT_TIMEOUT, lookup_host(connect_addr))
        .await
        .map_err(|_| format!("DNS lookup timed out after {TCP_CLIENT_CONNECT_TIMEOUT:?}"))?
        .map_err(|error| format!("DNS lookup failed: {error}"))?
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err("DNS lookup returned no socket addresses".to_string());
    }

    let mut failures = Vec::new();
    for address in addresses {
        match tokio::time::timeout(TCP_CLIENT_CONNECT_TIMEOUT, TcpStream::connect(address)).await {
            Ok(Ok(stream)) => return Ok(stream),
            Ok(Err(error)) => failures.push(format!("{address}: {error}")),
            Err(_) => failures.push(format!(
                "{address}: connect timed out after {TCP_CLIENT_CONNECT_TIMEOUT:?}"
            )),
        }
    }
    Err(format!(
        "all resolved socket addresses failed: {}",
        failures.join("; ")
    ))
}

async fn tcp_endpoint_reachable(connect_addr: &str) -> bool {
    connect_tcp_endpoint(connect_addr).await.is_some()
}

async fn any_tcp_endpoint_reachable(endpoints: &[String]) -> bool {
    for endpoint in endpoints {
        if tcp_endpoint_reachable(endpoint).await {
            return true;
        }
    }
    false
}

async fn unregister_tcp_client_endpoint(
    active_interface_registry: &ActiveInterfaceRegistry,
    status: &Arc<Mutex<NodeStatus>>,
    bus: &EventBus,
    endpoint: &str,
) {
    let removed = {
        let mut registry = active_interface_registry.lock().await;
        let remove_keys = registry
            .iter()
            .filter_map(|(interface, registered)| {
                (registered.label == endpoint).then_some(*interface)
            })
            .collect::<Vec<_>>();
        remove_keys
            .into_iter()
            .filter_map(|interface| registry.remove(&interface))
            .collect::<Vec<_>>()
    };
    for mut removed in removed {
        removed.state = "disconnected".to_string();
        publish_interface_registry_snapshot(active_interface_registry, status, bus, Some(removed))
            .await;
    }
}

fn set_runtime_interface_readiness(
    status: &Arc<Mutex<NodeStatus>>,
    bus: &EventBus,
    id: &str,
    state: RuntimeReadinessState,
    detail: String,
    last_error: Option<String>,
) {
    if let Ok(mut guard) = status.lock() {
        guard.set_interface_readiness(id, state, detail, last_error);
        bus.emit(NodeEvent::StatusChanged {
            status: guard.clone(),
        });
    }
}

fn spawn_tcp_client_interface_manager(
    transport: Arc<Transport>,
    connect_addr: String,
    active_interface_registry: ActiveInterfaceRegistry,
    status: Arc<Mutex<NodeStatus>>,
    bus: EventBus,
) {
    tokio::spawn(async move {
        let active = Arc::new(AtomicBool::new(false));
        loop {
            if active.load(Ordering::Acquire) {
                tokio::time::sleep(TCP_CLIENT_INTERFACE_RETRY_INTERVAL).await;
                continue;
            }

            if let Some(stream) = connect_tcp_endpoint(connect_addr.as_str()).await {
                info!(
                    "tcp_client: starting connected interface for <{}>",
                    connect_addr
                );
                active.store(true, Ordering::Release);
                let active_for_task = active.clone();
                let task_addr = connect_addr.clone();
                let registry_for_task = active_interface_registry.clone();
                let status_for_task = status.clone();
                let bus_for_task = bus.clone();
                let iface = transport.iface_manager().lock().await.spawn(
                    TcpClient::new_from_stream(connect_addr.clone(), stream),
                    move |context| async move {
                        TcpClient::spawn(context).await;
                        unregister_tcp_client_endpoint(
                            &registry_for_task,
                            &status_for_task,
                            &bus_for_task,
                            task_addr.as_str(),
                        )
                        .await;
                        active_for_task.store(false, Ordering::Release);
                        info!("tcp_client: stopped interface for <{}>", task_addr);
                    },
                );
                let status_update = new_interface_status(iface, connect_addr.clone(), "connected");
                active_interface_registry
                    .lock()
                    .await
                    .insert(iface, status_update.clone());
                publish_interface_registry_snapshot(
                    &active_interface_registry,
                    &status,
                    &bus,
                    Some(status_update),
                )
                .await;
                info!(
                    "tcp_client: connected interface endpoint=<{}> iface={}",
                    connect_addr, iface
                );
            }

            tokio::time::sleep(TCP_CLIENT_INTERFACE_RETRY_INTERVAL).await;
        }
    });
}

fn spawn_tcp_client_readiness_monitor(
    endpoints: Vec<String>,
    status: Arc<Mutex<NodeStatus>>,
    bus: EventBus,
) {
    if endpoints.is_empty() {
        return;
    }

    tokio::spawn(async move {
        let mut data_path_down = false;
        loop {
            let reachable = any_tcp_endpoint_reachable(endpoints.as_slice()).await;
            if reachable {
                if data_path_down {
                    info!(
                        "tcp_client: Reticulum TCP data path restored endpoints={}",
                        endpoints.join(",")
                    );
                    emit_operational_notice(
                        &bus,
                        LogLevel::Info {},
                        format!("Reticulum TCP data path restored: {}", endpoints.join(",")),
                    );
                    if let Ok(mut guard) = status.lock() {
                        guard.refresh_readiness();
                        bus.emit(NodeEvent::StatusChanged {
                            status: guard.clone(),
                        });
                    }
                }
                data_path_down = false;
            } else if !data_path_down {
                let message = tcp_data_path_unavailable_message(endpoints.as_slice());
                warn!("{}", message);
                bus.emit(NodeEvent::Error {
                    code: "NetworkError".to_string(),
                    message: message.clone(),
                });
                emit_operational_notice(
                    &bus,
                    LogLevel::Warn {},
                    format!(
                        "Reticulum TCP data path unavailable: {}",
                        endpoints.join(",")
                    ),
                );
                set_runtime_interface_readiness(
                    &status,
                    &bus,
                    "tcp",
                    RuntimeReadinessState::Failed,
                    "Configured TCP endpoints are unreachable".to_string(),
                    Some(message),
                );
                data_path_down = true;
            }

            tokio::time::sleep(TCP_CLIENT_READINESS_CHECK_INTERVAL).await;
        }
    });
}
