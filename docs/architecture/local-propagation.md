# Local propagation integration

Implementation is in progress against the approved [routing plan](../plans/local-propagation-routing.md).
Phase 0 was explicitly waived by the user; no airtime measurement is claimed.

`lxmf-propagation` owns admission, encrypted payloads, deduplication, quota, expiry,
eviction and retirement. Shared standard `/get` and `/offer` policy and existing
daemon SQL/schema/validation/retention are extracted from LXMF-rs daemon modules.
Daemon adapters retain their writer, counters and bounded peer inventory. The
embedded feature imports neither RPC dispatch nor ZeroMQ and opens no relay
service. REM never admits third-party deposits.

The existing Rust runtime creates one persistent service with its transport and
identity. It prepares one signed wire and recipient transient, tries normal direct
delivery, and commits locally when bounded delivery cannot complete. Unknown
recipient encryption identities fail with `UnresolvedRecipient`. The direct deadline
includes send-slot contention: 60 seconds for TCP, 180 for RNode. Remote relays use
Reticulum paths over any bearer, including LoRa through a transport node with TCP.
Having a TCP interface on the sender is not a prerequisite.

Once committed, compiled custody is the only retry owner. REM stops its prior
acknowledgement retry ownership, and telemetry records the owning immutable message
ID so route changes, repeated fixes and restart cannot submit that same publication
again. A new fix can supersede the prior payload. Expiry remains seven days from
first admission, or the shorter telemetry deadline. Capacity is 25 MiB of encrypted
recipient and sender recovery payload; SQLite, journal and status overhead are
bounded separately.

Existing runtime events and maintenance drive recovery. Verified recipient announces,
path responses and identified inbound/outbound links trigger direct recovery. Relay
recovery uploads the original transient. Work is ordered by next-attempt time and
recipient-triggered recovery selects that recipient. Power saver suspends background
work while allowing event-triggered direct recovery. Proactive warm sessions stay
disabled on RNode-only networks.

One native transfer permit covers propagation upload and retrieval. A retained send
owner checks current permissions after Link activation, holds the permit after caller
cancellation, and observes native Resource cancellation before releasing it. Upload
admission is persisted immediately before payload dispatch, not while merely trying
to open a Link. Uncertain uploads become `UploadedUnverified` and retain local
payloads. Resource transport completion is not durable relay-storage proof. Recipient
proof retires the payload atomically; negative application results do not become
accepted acknowledgements.

UDL, JNI, Java service/Capacitor, node-client and chat project `StoredLocally`,
`Uploading`, `UploadedUnverified`, `RelayHolds`, `Delivered`, `Acknowledged`, `Expired`
and `Evicted`. Terminal notices are acknowledged to custody only after durable
consumer projections succeed. Replay reconciles unchanged persisted notices too.
Retry rows use the latest wire ID as well as their logical message ID. The UI keeps
expiry/eviction visible and states that a relay copy may survive local expiry.

## Constrained retrieval

The provisional LoRa interval is 300 seconds, with one fetched message per poll
and a 2048-byte cap on each inventory or payload response. These remain
conservative unmeasured values following the Phase 0 waiver. An unpaginated
inventory exceeding the native cap defers retrieval; it never counts as empty.
Larger messages stay at the relay for a later unconstrained connection.

The approved relay correction keeps `/get` wants retryable after response
construction, size exclusion, interruption or cancellation. Only authenticated
receiver `haves` retire recipient custody. Peering `/offer`, schema and
recipient isolation remain unchanged. Stock Python already retains these wants;
older Rust relays need this correction for safe retries. Existing historical
completion marks are not rewritten by this amendment.

The compiled LXMF-rs client uses native retained RequestReceipts and Resource
limits, fetches one correlated transient at a time, verifies the original sender,
and calls the native application importer. Only durable import sends `haves`;
its control response is awaited. Rust's `true` and Python's `[]` responses are
accepted. A retained task holds the shared transfer permit through native Link
cleanup even after caller cancellation. REM's former control codecs and crypto
compatibility implementation are removed; existing response wrappers and legacy
salt support live in the compiled library.

Chat must be durably saved and free of mission-envelope fields. Supported single
EAM, event and telemetry operations, complete checklist batches and complete
marker batches can confirm. Partial, malformed, mixed or otherwise unproven
bundles remain relay-held. A supported idempotent operation counts as saved;
unknown handling and database failures do not. SOS confirmation remains deferred
until all of its separate projections can prove persistence.

Sender verification uses native identity lookup and a bounded path request when
needed. REM does not persist public signing keys today: after restart, an offline
sender without an available cached announce cannot yet be verified. Its message
stays at the relay until identity resolution succeeds. This amendment adds no new
identity cache, schema or polling loop.

The Phase 1 library-only `retrieve_for` API is available without enabling a phone
service. It derives the recipient from a host-authenticated Link identity, checks
current sharing permission against the original wire, and returns one bounded
recipient ciphertext while retaining custody. A guard holds the shared transfer
admission. Isolation, expiry, denial, budget retry and admission ownership are tested.

Optional phone-hosted `rem.fetch` retrieval remains unimplemented until Phase 2
field evidence demonstrates asymmetric reachability. No new `lxmf.propagation`
destination or phone relay announce is introduced.
