/// A TEAM directory refresh failure; `Rejected` keeps the reason RCH gave so operators can act on it.
enum HubDirectoryRefreshError {
    Node(NodeError),
    Rejected(String),
}

impl From<NodeError> for HubDirectoryRefreshError {
    fn from(error: NodeError) -> Self {
        Self::Node(error)
    }
}

impl HubDirectoryRefreshError {
    fn node_error(&self) -> NodeError {
        match self {
            Self::Node(error) => *error,
            Self::Rejected(_) => NodeError::NetworkError {},
        }
    }
}

async fn publish_failed_hub_directory_refresh(bus: &EventBus, error: &HubDirectoryRefreshError) {
    let message = match error {
        HubDirectoryRefreshError::Node(error) => format!(
            "RCH TEAM peer directory refresh failed; retaining the last successful directory: {error}"
        ),
        HubDirectoryRefreshError::Rejected(reason) => format!(
            "RCH refused the TEAM peer directory request: {reason}; retaining the last successful directory"
        ),
    };
    bus.emit(NodeEvent::Error {
        code: node_error_code(&error.node_error()).to_string(),
        message,
    });
}
