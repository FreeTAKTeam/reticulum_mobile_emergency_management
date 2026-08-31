fn tcp_endpoint_connect_addr(endpoint: &str) -> &str {
    endpoint
        .trim()
        .strip_prefix("tcp://")
        .unwrap_or_else(|| endpoint.trim())
}

fn configured_tcp_client_endpoints(endpoints: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for endpoint in endpoints {
        let connect_addr = tcp_endpoint_connect_addr(endpoint).trim();
        if connect_addr.is_empty() || normalized.iter().any(|value| value == connect_addr) {
            continue;
        }
        normalized.push(connect_addr.to_string());
    }
    normalized
}

fn tcp_endpoint_host(connect_addr: &str) -> &str {
    connect_addr
        .rsplit_once(':')
        .map(|(host, _)| host)
        .unwrap_or(connect_addr)
        .trim_matches(['[', ']'])
        .trim()
}

fn tcp_endpoint_is_loopback(connect_addr: &str) -> bool {
    let host = tcp_endpoint_host(connect_addr).to_ascii_lowercase();
    host == "localhost" || host == "::1" || host.starts_with("127.")
}

fn tcp_readiness_monitor_endpoints(endpoints: &[String]) -> Vec<String> {
    endpoints
        .iter()
        .filter(|endpoint| !tcp_endpoint_is_loopback(endpoint))
        .cloned()
        .collect()
}

fn tcp_data_path_unavailable_message(endpoints: &[String]) -> String {
    format!(
        "transport startup failed: no reachable Reticulum TCP interface endpoints={}",
        endpoints.join(",")
    )
}
