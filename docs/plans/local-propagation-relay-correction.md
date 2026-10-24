# Proposed correction: preserve relay messages until receiver confirmation

Status: proposed; not applied. This changes one constraint of the approved local-propagation plan.

The approved plan says: "Don't change the schema, the custody semantics or the bounded inventory processing."
The extracted Rust `/get` handler currently marks a wanted message `transferred` before its response is delivered, and marks a too-large wanted message `transfer_limited`. Both suppress later retrieval for that receiver. A native size limit, cancellation or radio interruption cannot undo those server-side marks. The inventory contains hashes without message sizes, so requesting one hash at a time cannot avoid the issue.

## Smallest correction

- Keep authentication, filtering, inventory bounds, transfer-size limits and the existing schema unchanged.
- Stop marking wanted payloads complete or permanently transfer-limited while constructing a `/get` response.
- Retain the existing authenticated `haves` processing as the receiver-confirmed retirement path.
- Update only daemon tests that assert premature completion/transfer-limit marks; add interrupted-transfer and larger-limit retry regressions.
- Implement bounded native retrieval: cap inventory responses, fetch one payload at a time, durably import it before sending `haves`, and retain request ownership through cancellation/teardown.

This follows the pinned stock Python LXMF handler's receiver-confirmed behavior without adding a new wire protocol or importing the daemon. It deliberately changes the Rust daemon's current custody semantics, so it needs an explicit amendment to the approved plan before implementation.

## Evidence and current protection

- Shared Rust handler: `crates/libs/lxmf-propagation/src/handlers/get.rs` in the sibling LXMF-rs implementation worktree.
- The Rust inventory returns transient hashes; its smallest-first ordering does not prove any requested message fits.
- Native request size caps prevent oversized Resource allocation but cannot restore a server-side custody mark already written.
- REM currently rejects capped `/get` before network dispatch. Direct recovery, local custody, relay upload and ordinary TCP retrieval remain implemented. Automatic bounded LoRa retrieval is incomplete, and no new release is qualified by this checkpoint.
