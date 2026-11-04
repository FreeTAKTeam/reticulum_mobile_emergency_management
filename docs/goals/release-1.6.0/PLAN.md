# REM 1.6.0 performance release

**Intent:** Publish a signed stable REM 1.6.0 release focused on CPU/battery efficiency, with the goTenna Mesh plugin explicitly beta and More → Quit included.
**Current Behavior:** Latest public REM is 1.5.7; main contains the measured announce/peer optimization and goTenna bridge, Quit is reviewed but uncommitted, and Complete packaging accepts exactly three older plugins.
**Expected Outcome:** Complete APK exports four optional signed installers including goTenna beta; Light APK/AAB plus separate installers are available, upgrade signatures are preserved, and Quit stops networking/background service before closing Android.
**Target-Perspective Output:** A user can download verified 1.6.0 assets, open the Complete build offline to obtain four matching plugin files, identify goTenna as Beta in Android/REM/configuration, and use More → Quit. Release notes lead with measured performance improvements and explain qualification limits.
**Truth Owner:** Native runtime owns networking and power behavior; Android owns service/installer lifecycle; the existing release script owns package composition and signing; release metadata records exact source and artifact hashes.
**Contract Boundary:** Existing plugin API 1.5, native stop operation, Capacitor activity close, APK signatures/package IDs, four checksum-named bundled installers, and GitHub release assets.
**Cutover:** Publish 1.6.0 from clean isolated source, rebuild every REM Android ABI against committed LXMF/RCH pins, and extend the existing three-installer packaging path to exactly four with goTenna beta.
**Displaced Path:** Three-plugin count/whitelist becomes four; no parallel packager, stale JNI libraries, private R3S code, or debug-signed artifacts enter the release.
**Value Density:** Reuse measured optimization commit5c88839, the reviewed Quit flow, existing signing identity, offline exporter, packaging script and test facilities.
**Acceptance Evidence:** Hosted source CI; final native tests and release benchmarks; APK/AAB package IDs, version, signer continuity and 16 KiB alignment; exact bundled/standalone byte equality; offline Android35 emulator export/install/metadata/Quit observations; published download checksums.
**Evidence Lane:** Final software/package/emulator checks plus clearly labelled prior Pixel CPU measurements. No claimed discharge-duration gain, physical goTenna delivery/range, new phone-radio qualification, long soak or iOS build.
**Kill Criteria:** No dirty-source dependency/JNI reuse; no unqualified battery-life percentage; no silent plugin-count mismatch; no secret material in Git/release assets; beta must be visible; do not publish if source/package verification has unresolved blockers.
**Architecture Slice:** Isolated release checkout; Quit source/tests only from the dirty workspace; goTenna version/labels, bundled exporter/tests, existing offline-release script, package version metadata and release documentation. Preserve all unrelated telemetry changes.
**Plan Review Gate:** PRE read-only review before release edits; POST correctness/maintainability and artifact verification before publication.

## Tasks

1. Main: isolate source at main b80c0f0; include reviewed Quit files only; pin LXMF ea30c23d and RCH979e8fbb; confirm measured battery patch is already in main. Explorer is read-only.
2. Main: label goTenna0.2.0-beta.1, update existing packaging/exporter to four plugins, bump REM metadata1.6.0, add regression coverage and release notes. No unrelated runtime refactor.
3. Main: validate narrow source/native/UI/Android checks, commit/push release branch, create and merge a reviewed release PR after source CI passes. Keep all source mutations sequential.
4. Main: use ignored signing configuration and existing REM release key, pinned ATN1c3ac84c and clean dependencies to build fresh three-ABI Complete/Light APKs and Light AAB. Generate checksums/provenance and validate installer equality, signer continuity, alignment and UI metadata. Build artifacts remain ignored.
5. Main with read-only reviewer/verifier: inspect final release artifacts in isolated emulator, review plan/correctness/maintenance alignment, create draft GitHub release with exact assets, verify downloaded bytes, then publish stable1.6.0 and report residual qualification limits.

## Execution boundaries

- Allowed source writes: Quit files, plugin beta metadata/config copy, BundledPluginExporter and focused tests, tools/build-offline-release.sh, package metadata/lock and release docs.
- Forbidden: unrelated dirty R3S files, live LXMF workspace, signing secrets in tracked files, generated bindings by hand, user-device installation or public radio transmission.
- Build source commit must equal tag/source recorded in manifests. If a code fix follows packaging, rebuild affected artifacts and reverify before publication.
- User request authorizes the release and necessary commit/push/PR/merge/publication steps; no extra approval pause is inferred.

## PRE review resolution

Read-only PRE review: aligned, no blocker or major finding. Require a versionCode greater than1.5.7 and identical across host packages; exactly three intended ABIs and packaged-library hashes/build IDs matching fresh outputs; mandatory Kotlin regeneration from clean UDL; signed LightAAB in provenance and download verification; both ELF LOAD and APK ZIP16KiB alignment. Build/tag must retain the same frozen source SHA; packaging may run while its hosted CI is pending, but publication waits for successful CI and PR integration.
