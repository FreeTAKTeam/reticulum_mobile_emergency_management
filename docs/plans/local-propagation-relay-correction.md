# Approved correction: preserve relay messages until receiver confirmation

Status: explicitly approved by the user and implemented; host and Android verification pass. This amends the preserve-custody-semantics constraint of the local-propagation plan.

The approved plan says: "Don't change the schema, the custody semantics or the bounded inventory processing."
The extracted Rust `/get` handler currently marks a wanted message `transferred` before its response is delivered, and marks a too-large wanted message `transfer_limited`. Both suppress later retrieval for that receiver. A native size limit, cancellation or radio interruption cannot undo those server-side marks. The inventory contains hashes without message sizes, so requesting one hash at a time cannot avoid the issue.

## Smallest correction

- Keep authentication, filtering, inventory bounds, transfer-size limits and the existing schema unchanged.
- Stop marking wanted payloads complete or permanently transfer-limited while constructing a `/get` response.
- Retain the existing authenticated `haves` processing as the receiver-confirmed retirement path.
- Update only daemon tests that assert premature completion/transfer-limit marks; add interrupted-transfer and larger-limit retry regressions.
- Implement bounded native retrieval: cap inventory responses, fetch one payload at a time, durably import it before sending `haves`, and retain request ownership through cancellation/teardown.

This follows the pinned stock Python LXMF handler's receiver-confirmed behavior without adding a new wire protocol or importing the daemon. The user explicitly approved this change to the Rust daemon custody semantics. Authentication, schema and peering `/offer` semantics remain unchanged.

## Evidence and implementation

- Shared Rust handler: `crates/libs/lxmf-propagation/src/handlers/get.rs` in the sibling LXMF-rs implementation worktree.
- The Rust inventory returns transient hashes; its smallest-first ordering does not prove any requested message fits.
- Native request size caps prevent oversized Resource allocation but cannot restore a server-side custody mark already written.
- REM now delegates TCP and bounded LoRa retrieval to the compiled native client. Failed decrypt, signature verification or durable import leaves relay custody intact. The new implementation still requires physical routing acceptance before release.
- Unpaginated inventories beyond the cap and unsupported or partial application bundles are deferred safely. Sender identity resolution is bounded; no new persistent identity cache is included.
