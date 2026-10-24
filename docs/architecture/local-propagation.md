# Offline local propagation

The [offline-only correction](../plans/offline-only-correction.md) governs this
implementation. Normal delivery, Connect, alias handling, caller deadlines,
packet sizing, regular relay Sync and automatic polling preserve release 1.5.5:
REM `8070416`, LXMF-rs `7822b608`.

REM runs that existing delivery policy first. Successful submissions return the
original report without writing local custody. At a missing-route failure, or a
native attempt explicitly classified as unreachable, the offline fallback checks
current sharing permission and validated recipient identity, then deposits once.
An absent open Link alone never selects custody. Busy, validation, permission,
internal and storage failures do not establish that a peer is offline.

The compiled runtime captures a failed attempt's signed wire on the existing SDK
blocking thread. Admission validates that exact wire's sender signature and
recipient. If no wire was constructed, preparation occurs once at offline admission.
Baseline retries remain unchanged; there is no second direct/relay attempt loop
in the custody fallback. Existing timing may exceed the original plan's proposed
60/180-second targets; preserving tested online behavior takes precedence.

The shared `lxmf-propagation` crate owns encrypted storage, quota, deduplication,
expiry, eviction and retirement. It imports neither daemon dispatch nor ZeroMQ,
registers no phone relay endpoint and rejects foreign messages. Existing daemon
storage and standard `/get`/`/offer` handlers delegate to this crate. The approved
relay correction retains requested messages until authenticated receiver `haves`.

After durable deposit, Core is the only retry owner. Legacy timers are removed
before fallible status publication. Chat history is saved before deposit, and Core
alone advances its custody status. Telemetry retains its immutable owner for a fix;
a new fix may supersede it. Limits remain 25 MiB and seven days, or a shorter
telemetry expiry. Expiry and eviction remain visible to the user.

Existing announces, learned paths, identified Links and maintenance drive only
already-stored work. Direct recovery preserves the signed wire; relay upload
preserves its recipient transient. Reticulum can reach a relay through LoRa to a
transport-enabled node with TCP. The sender need not have TCP. Uncertain relay
uploads retain their local copy as `UploadedUnverified`.

A retained task owns each offline transfer through completion or exact Resource
cancellation, identified by original hash and Link ID. It never resets a shared
online Link. Current sharing permission is checked again before dispatch. Empty
custody stores schedule no recovery traffic. Power saver suspends background work.

Normal LoRa-only automatic relay polling remains disabled, as in the tested
release; the config reports interval zero. Normal manual Sync also remains the
tested path. The broader replacement retrieval client has been removed from this
correction. The library-only authenticated `retrieve_for` API remains additive;
optional phone-hosted Phase 3 retrieval is deferred.

Host tests qualify storage and the offline boundary separately from physical
acceptance. No new release is qualified until the rebuilt scoped candidate passes
the required hardware routing matrix.
