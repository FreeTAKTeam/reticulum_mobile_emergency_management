# REM goTenna Mesh plugin

Experimental Android plugin **0.2.0** for goTenna Mesh over Bluetooth LE. Requires the updated REM native host with plugin API **1.5** from this work.

Chat is always addressed to a person. Each Mesh chat frame carries the sender's reported REM hash, the intended recipient's 32-digit LXMF destination hash, and a stable message ID. Received chat for this REM destination enters **REM's ordinary conversation history**. A chat for another destination can be forwarded through compiled LXMF to that carried recipient, under the gateway's own identity and with original-source attribution. There is no plugin chat inbox and no global forwarding recipient.

Events travel as canonical **MECP event codes**, such as `MECP/2/P01`, rather than a display label or command name. Both locally persisted events and events received through other REM interfaces enter the Mesh relay feed. Received Mesh events enter REM's native event timeline and replication workflow. Positions continue to use the existing shared map marker service.

Mesh headers are plaintext, and carried sender hashes are **reported, not authenticated**. Incoming Mesh chat has a visible unverified-sender badge. Imported events use the gateway's actual native identity with unverified goTenna attribution; they do not create trusted peer identities. Radio acceptance is displayed as delivery unconfirmed.

## Setup

1. Build/install the matching updated REM host and plugin. Refresh **Settings → Plugins**, approve the publisher and enable it.
2. Grant **Bridge addressed chat, event codes and positions** for chat/relay, **Publish events** for event import, **Add and edit its own map markers** for positions, and **Read this device's location** for manual position sends. **Send LXMF** additionally permits forwarding chat addressed to a nonlocal recipient.
3. Open configuration, select **Pair radio**, grant Bluetooth permissions and choose your Mesh in pairing mode. Android 8–11 also needs location services for BLE discovery. Close other goTenna connection owners before connecting.
4. Enter an existing goTenna GID you control (12 hexadecimal digits), save and connect. This plugin does not provision/reset radio identities.
5. To send chat explicitly over Mesh, enter the recipient's LXMF destination in the per-message recipient field. The outgoing chat is also recorded in REM's normal history. The existing REM composer retains its normal Reticulum route; this plugin is an application bridge, not a general Reticulum transport.
6. Enable **REM → public goTenna** to forward new received chat, MECP event codes and positions from other interfaces. This defaults off. A recipient hash provides addressing, not radio encryption. Enabling/connecting starts at the feed's current tail to avoid broadcasting history.

## Mesh application frames

Every addressed fragment is UTF-8 text inside a Mesh Shout, at most 180 bytes:

```text
REM1 C <id32> <reported-source32> <destination32> <part>/<count> <chat-text>
REM1 E <id32> <reported-source32> - <part>/<count> <MECP-code>
```

Hashes and IDs use lowercase hexadecimal. Chat without a valid destination is rejected as chat; ordinary legacy Shouts do not become REM conversation entries. Event codes retain severity and code, such as `MECP/0/B01` or `MECP/2/P01+W02`; descriptive extras are not sent in place of the code. API 1.5 operations are documented in [the host contract](../../docs/plugins/android-plugin-system.md#communication-relay-15).

At most 24 fragments and 16 assemblies are kept, with a two-minute assembly lifetime. Fragment boundaries preserve Unicode characters. Conflicting duplicate fragments are rejected. Assembly snapshots are persisted before the radio advances its queue; original expiry times survive restart. Completed inbound admissions remain in one durable pending slot until the native host accepts them. Invalid payloads are rejected visibly rather than blocking later reception. Chat for another person is ignored when forwarding is not granted. After a host/grant/routing error, settings show a legitimate pending reception; saving settings or reconnecting retries the same ID. Uncertain nonlocal submissions are explicitly unconfirmed and are not sent again automatically. Manual outgoing chat is recorded as an unconfirmed attempt before radio submission; a separate durable receipt slot records later radio acceptance without retransmitting bytes.

Raw received bytes are kept only for internal processing custody, bounded to 4 MiB. They are never a separate user inbox. Existing archived bytes are retained. Chat/event acceptance and stable IDs are owned by the native runtime. Outgoing Mesh fragments recheck grants, keep a bounded attempted-send ledger and do not blindly retry uncertain radio sends. Imported event IDs and forwarded chat provenance survive native replication and prevent the inspected radio/network feedback paths. Deduplication is per native gateway; several gateways can each forward the same reception.

## Other limits

- The feed retains 2,048 entries and reports cursor gaps. At most one relay fragment is attempted per five-second cycle; work older than two minutes is skipped visibly.
- Manual chat is limited to 180 UTF-8 bytes before fragmentation. Larger received REM chat is bounded by the 24-fragment limit and rejected visibly if it will not fit.
- Positions use the existing Mesh location attachment. Imported fixes remain unverified; older fixes cannot move newer markers backward. Ordering tracks up to 256 reported GIDs.
- Legacy/encrypted/unsupported messages remain internal raw custody and do not appear as unaddressed chat.
- Firmware-specific discovery, GATT, radio delivery/range, device power-loss and real multi-gateway behavior have not been physically qualified. There is no automatic recipient-delivery claim.

## Build and verify

Set `REM_HOST_FINGERPRINTS` to the actual **host APK's** public SHA-256 signing fingerprint; the build passes it as `remHostFingerprints`. Empty fingerprints reject all Binder callers.

```sh
GOTENNA_RELAY_FIXTURE="$PWD/output/gotenna-relay.json" \
  cargo test --manifest-path crates/reticulum_mobile/Cargo.toml plugin_relay

GOTENNA_RELAY_FIXTURE="$PWD/output/gotenna-relay.json" \
  bash apps/mobile/android/gradlew -p apps/mobile/android \
  -PremHostFingerprints="$REM_HOST_FINGERPRINTS" \
  :gotenna-plugin:testDebugUnitTest :gotenna-plugin:lintDebug :gotenna-plugin:assembleDebug
```

APK: `android/build/outputs/apk/debug/gotenna-plugin-debug.apk`. Release builds require ignored `apps/mobile/android/gotenna-keystore.properties` using the existing plugin signing format. This plugin is not bundled in the current three-plugin offline release package.

Protocol references: [sybip/pyGT](https://github.com/sybip/pyGT) and local `gotenna-test` wire vectors. See `THIRD_PARTY_NOTICES` and [execution evidence](../../docs/goals/gotenna-plugin/EVIDENCE.md).
