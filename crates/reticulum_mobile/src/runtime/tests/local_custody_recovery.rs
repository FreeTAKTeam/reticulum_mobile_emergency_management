#[test]
fn stored_message_can_leave_a_stale_direct_path_for_a_relay() {
    use lxmf_runtime::propagation_store::{MessageKind, State, Status};
    let mut stored = Status {
        transient_id: "aa".repeat(32),
        message_id: "bb".repeat(32),
        destination: "cc".repeat(16),
        state: State::StoredLocally,
        admitted_at: 10,
        expires_at: 100,
        encrypted_bytes: 200,
        uploaded: false,
        relay: None,
        attempts: 0,
        next_attempt_at: 0,
        kind: MessageKind::Chat,
        application_key: None,
    };
    assert!(
        local_custody_should_try_direct(&stored, true, false, true),
        "first attempt prefers direct"
    );
    stored.attempts = 1;
    assert!(
        !local_custody_should_try_direct(&stored, true, false, true),
        "unconfirmed cached path cannot starve the relay"
    );
    assert!(
        local_custody_should_try_direct(&stored, true, false, false),
        "a cached relay with no route must not preempt a usable recipient path"
    );
    stored.attempts = 2;
    assert!(
        local_custody_should_try_direct(&stored, true, false, true),
        "a failed relay turn permits direct recovery on the next scheduled attempt"
    );
    stored.attempts = 3;
    assert!(
        !local_custody_should_try_direct(&stored, true, false, true),
        "both stale routes receive bounded turns"
    );
    assert!(
        local_custody_should_try_direct(&stored, true, true, true),
        "fresh recipient event prefers direct"
    );
    stored.uploaded = true;
    assert!(
        local_custody_should_try_direct(&stored, true, false, true),
        "an uncertain relay copy does not prevent recipient proof"
    );
    assert!(
        !local_custody_should_try_direct(&stored, false, false, true),
        "do not invent a direct route"
    );
}
