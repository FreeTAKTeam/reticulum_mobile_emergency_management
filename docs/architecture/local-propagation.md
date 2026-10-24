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

## Constrained retrieval gate

The provisional LoRa interval is 300 seconds and transfer budget is 2048 bytes;
these are conservative unmeasured values following the Phase 0 waiver. Uploads use the budget. Safe constrained remote retrieval is not complete.

Pinned Python `lxmd` skips oversized `/get` wants without deleting them, but its
inventory is unpaginated. Current Rust relay policy marks individually oversized
wants transfer-limited and fetched wants transferred before recipient import;
those marks can suppress later retrieval. This pre-existing daemon behavior was
preserved during extraction, as required by the plan. REM therefore refuses a
capped `/get` before transmitting; it does not risk hiding larger remote messages.
TCP retrieval retains its existing behavior. Native response-size enforcement can
bound inventory reception, but cannot undo the Rust relay's completion marks.
A compatible safe retrieval contract or an explicitly revised policy is needed
before this part of Phase 2 or the release gate can pass.

Optional phone-hosted `rem.fetch` retrieval remains unimplemented until Phase 2
field evidence demonstrates asymmetric reachability. No new `lxmf.propagation`
destination or phone relay announce is introduced.
