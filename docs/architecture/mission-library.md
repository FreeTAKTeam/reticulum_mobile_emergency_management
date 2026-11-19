# Mission Library

[REM #356](https://github.com/FreeTAKTeam/reticulum_mobile_emergency_management/issues/356)
(offline Library) and [REM #358](https://github.com/FreeTAKTeam/reticulum_mobile_emergency_management/issues/358)
(peer sharing), part of the
[Mission Library sequence](https://github.com/FreeTAKTeam/rreticulum-package-protocol/issues/2).

## Ownership

| Layer | Owns |
|---|---|
| Rust `app_state/library*.rs` | `rem_library_items` (shared `r3akt-library-core` item JSON), `rem_library_shares`, `rem_library_offers`; bytes in `<storage>/library/<sha256>` |
| Rust `runtime/library_share.rs` | `rpp_core::rns_share::ShareNode`: `rpp.receive` inbox, publishing, serving, fetching |
| JNI `jni_bridge/library_api.rs` | JSON calls: import, list, get, delete, set missions, share, download |
| Java `ReticulumNodeLibraryPluginApi` | Storage Access Framework picker, private cache copy, FileProvider open, 256 px thumbnails |
| Vue `LibraryView`, `libraryStore` | Filters, selection and intent only; no hashing, integrity or network policy |

## Storage rules

- An import streams the picked file into `<sha256>.part`, hashes it, fsyncs it and renames it into place. Leftover `.part` files are deleted on start.
- Identical content returns the existing item (`local:<first 32 hex of sha256>`).
- Imports are capped at 64 MiB. Names must be a single safe file name.
- Availability is recomputed from storage. A local item whose bytes vanished shows as `failed` and cannot be opened.
- Remove deletes only the local index row, then the bytes once no other item refers to them. For an Offer, Remove only forgets it.

## Peer sharing (RPP 0.3 RC1)

1. **Share** (local item, then a saved peer):
   - publishes the file once as a signed one-file Package with its RDO Package record;
   - lets only that peer READ and pull it;
   - sends an Offer to the peer's `rpp.receive` inbox.

   The publication is persisted and republished on start.
2. **Receiving an Offer:**
   - The receiving inbox resolves the Package owner through RDO and requires the sender to be that owner.
   - It answers `defer` and lists the Offer under **Shared With Me** as a remote item.
   - Nothing downloads automatically.
3. **Download** (operator action only):
   - resolves the Package, READs the signed record over `/rdo/1/op`, pulls the missing chunks and verifies chunk, bundle, file and manifest hashes;
   - imports the promoted file as a `peer` item carrying its RPP identities and publisher.
   - Partial verified chunks stay under `library/rpp` for a retry.

Share and Download are refused in power saver. Transfers run as background tasks outside the SOS and acknowledgement command lanes. They never start on their own, so LoRa never begins a bulk transfer unasked.

## RCH source (REM #357)

1. **Refresh from hub** sends `mission.library.list` (field 9) to the configured hub. Answers (field 10) are trusted only from `hub_identity_hash`.
2. List pages are cached in `rem_library_hub_items` as metadata only, so they can be browsed offline. A rejection, such as an older RCH without the command, is stored as the hub Library status and shown in place of a list.
3. **Download** sends `mission.library.item.fetch` with REM's identity hash, and marks the item `downloading`.
4. The `ok` answer carries the Package Destination. REM pulls it with the same RPP path as peer sharing and imports the file as an `rch` item.
5. Refreshing never deletes downloaded files. Autonomous REM never asks the hub.

## Not yet covered

- Two-device acceptance (TCP, split TCP/LoRa, interruption above 100 chunks, disk full, old peers) has not been run.
- Received Offers do not show the file name until download, because RC1 Offers carry only identities.
