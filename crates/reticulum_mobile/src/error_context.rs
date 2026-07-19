use std::cell::RefCell;
use std::fmt::Debug;
use std::panic::Location;

use crate::types::NodeError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InternalFailure {
    pub(crate) code: &'static str,
    pub(crate) message: String,
    pub(crate) operation: String,
    pub(crate) retryable: bool,
    pub(crate) cause: String,
}

thread_local! {
    static LAST_INTERNAL_FAILURE: RefCell<Option<InternalFailure>> = const { RefCell::new(None) };
}

#[track_caller]
pub(crate) fn contextual_node_error<E>(error: NodeError, cause: E) -> NodeError
where
    E: Debug,
{
    let location = Location::caller();
    let operation = operation_from_location(location);
    let code = node_error_code(&error);
    let cause = format!("{cause:?}");
    let failure = InternalFailure {
        code,
        message: format!("{code} while performing {operation}"),
        operation,
        retryable: node_error_code_is_retryable(code),
        cause,
    };
    log::error!(
        "{} [{}] failed: {}",
        failure.operation,
        failure.code,
        failure.cause
    );
    LAST_INTERNAL_FAILURE.with(|slot| slot.replace(Some(failure)));
    error
}

pub(crate) fn clear_internal_failure() {
    LAST_INTERNAL_FAILURE.with(|slot| slot.replace(None));
}

pub(crate) fn take_internal_failure(code: &str) -> Option<InternalFailure> {
    LAST_INTERNAL_FAILURE.with(|slot| {
        let failure = slot.borrow_mut().take();
        failure.filter(|failure| failure.code == code)
    })
}

pub(crate) fn node_error_code_is_retryable(code: &str) -> bool {
    matches!(
        code,
        "IoError" | "NetworkError" | "ReticulumError" | "Timeout" | "EventStreamClosed"
    )
}

pub(crate) fn node_error_code(error: &NodeError) -> &'static str {
    match error {
        NodeError::InvalidConfig {} => "InvalidConfig",
        NodeError::IoError {} => "IoError",
        NodeError::NetworkError {} => "NetworkError",
        NodeError::ReticulumError {} => "ReticulumError",
        NodeError::AlreadyRunning {} => "AlreadyRunning",
        NodeError::NotRunning {} => "NotRunning",
        NodeError::Timeout {} => "Timeout",
        NodeError::LxmfWireEncodeError {} => "LxmfWireEncodeError",
        NodeError::LxmfMessageIdParseError {} => "LxmfMessageIdParseError",
        NodeError::LxmfPacketTooLarge {} => "LxmfPacketTooLarge",
        NodeError::LxmfPacketBuildError {} => "LxmfPacketBuildError",
        NodeError::EventStreamClosed {} => "EventStreamClosed",
        NodeError::InternalError {} => "InternalError",
    }
}

fn operation_from_location(location: &'static Location<'static>) -> String {
    location
        .file()
        .strip_prefix("crates/reticulum_mobile/")
        .unwrap_or(location.file())
        .strip_prefix("src/")
        .unwrap_or(location.file())
        .trim_end_matches(".rs")
        .replace('/', ".")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contextual_errors_keep_category_and_cause() {
        clear_internal_failure();

        let error = contextual_node_error(NodeError::IoError {}, "database unavailable");

        assert!(matches!(error, NodeError::IoError {}));
        let context = take_internal_failure("IoError").expect("context should be recorded");
        assert_eq!(context.code, "IoError");
        assert!(context.retryable);
        assert_eq!(context.cause, "\"database unavailable\"");
        assert!(context.operation.contains("error_context"));
    }
}
