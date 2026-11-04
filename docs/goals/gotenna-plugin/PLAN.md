# goTenna Mesh addressed chat and event-code correction

**Intent:** Preserve REM's addressed chat and MECP event semantics across Mesh BLE.
**Current Behavior:** Initial plugin displays text in a plugin inbox, forwards it to a fixed optional recipient, and omits events from its feed. This is rejected by the user.
**Expected Outcome:** Every chat frame carries an explicit person's LXMF destination hash. Received local chat enters REM's ordinary conversation store; nonlocal chat forwards through compiled LXMF to its carried recipient. Events send canonical MECP codes and enter the native event timeline. Position exchange remains available.
**Target-Perspective Output:** A goTenna chat addressed to this REM destination appears in that person's normal chat; another destination is never displayed as local chat. A REM event such as MECP/2/P01 is present on Mesh as its code, not a label. Plugin settings have no chat inbox or global forwarding recipient.
**Truth Owner:** Rust owns recipients, chat/event admission, persistence, provenance and replication. Plugin owns Mesh framing, fragmentation, BLE and internal durable receive custody. WebView only presents configuration and explicit addressed send requests.
**Contract Boundary:** API 1.5 JSON relay.poll, relay.identity, relay.chat.accept, relay.chat.pending, relay.chat.sent and relay.event.accept; current communicationsRelay grant gates chat ingress/recording, lxmfSend also gates nonlocal forwarding, eventsPublish also gates event import. Local native runtime command updates the existing messaging cache and projections through upsert_message_record. No public UniFFI changes.
**Cutover:** REM1 C <id32> <source32> <destination32> <part>/<count> <text>; REM1 E <id32> <source32> - <part>/<count> <MECP-code>. Full canonical code MECP/0..3/<code[+code]> is carried. At most 24 UTF-8-safe fragments and 16 in-progress assemblies, expiring after two minutes; changed duplicates are rejected. Earlier source-only [REM:] text frames and unaddressed Mesh text are not chat.
**Displaced Path:** Remove settings inbox, fixed forwardTo preference, plugin-only mesh_message LXMF envelopes, and broad source-only text relay. Keep the raw durable archive internal; do not delete existing received bytes.
**Value Density:** Reuse existing native chat store, compiled LXMF send, map APIs and native MECP expansion; no second LXMF protocol implementation or synthetic peer identity.
**Acceptance Evidence:** Native local-recipient/cache tests, wrong/missing recipient and current-grant rejection, attributed compiled forwarding, actual other-interface ingress recipient preserved, local/remote event-code feed, JVM addressed fragments/reassembly/conflicts/Unicode, app provenance badge/browser regression, APK/lint/source-size checks.
**Evidence Lane:** Software and packaging evidence. Physical radio, Android Binder/GATT and real cross-device delivery remain unproven.
**Kill Criteria:** No unaddressed chat import; no global default recipient; no authenticated-sender claim for unsigned Mesh headers; no raw archive as a user inbox; no event title in place of MECP code; no unbounded assemblies or automatic retry of uncertain transmission.
**Architecture Slice:** plugins/gotenna, native plugin relay modules, internal runtime import command, receive recipient projection, native event persistence feed, ConversationThread provenance badge and focused tests/docs. Preserve unrelated dirty telemetry work and the 500-line gates.
**Plan Review Gate:** PRE review of this corrected contract before native chat/event execution; POST correctness/maintainability/verifier review before finish.

1. Main: native recipient-aware ingress/recording, event code feed, internal command/cache update and contract tests.
2. Main: typed addressed Mesh frames, bounded reassembly, service routing, internal-only archive, per-message recipient send; remove obsolete inbox/global-recipient controls and fixtures.
3. Main: visible unverified provenance in normal chat; browser coverage and event/code fixtures; rebuild experimental APK and update docs/evidence.
4. Read-only reviewer/maintainer/verifier gates; fix material findings. No deployment, release publishing, generic Reticulum transport, cryptographic goTenna sender authentication or iOS work.

## PRE resolutions

- Native local chat IDs are a deterministic namespace of plugin/source/recipient/REM1 ID. Native forwarding claims are durable and scoped to that stable ID before compiled LXMF submission; uncertain submissions are never blindly repeated. Forwarded bodies retain a `[goTenna:...]` origin marker excluded from the radio feed. Imported events use stable `gotenna-<REM1-ID>` UIDs, retained through ordinary native replication and excluded from event re-export. Reported hashes never create trusted peers.
- `relay.chat.sent` forces the local sender and a delivery-unconfirmed radio status. The normal chat UI labels it “Radio accepted · delivery unconfirmed”, overriding the existing generic SentDirect→Delivered label for this provenance only. Event imports force the gateway's real identity and an unverified goTenna display attribution.
- PRE review aligned after these clauses. Implementation proceeds with the reviewed minimum attributed plaintext bridge; this is not a generic Reticulum interface.

## POST custody resolutions

- Partial assemblies and completed inbound admissions checkpoint atomically before radio queue advancement. Disconnect does not clear worker-owned assemblies. Snapshots restore original expiry times.
- Native rejection codes survive Binder request correlation. Terminal invalid/not-local payloads are rejected visibly; permission/routing/uncertain admissions retain custody and await operator retry.
- A normal outgoing chat record exists before radio writes. A separate durable pending-sent receipt upgrades only native history after acceptance, never blindly retransmitting radio bytes. Unknown forwarding attempts remain unconfirmed until an actual native submission result exists.
- Dedup is per gateway; this implementation does not claim network-wide exactly-once delivery across multiple independent gateways.
