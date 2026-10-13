# AGENTS.md

Guidance for coding agents working in `reticulum_mobile_emergency_management`.

## Project Snapshot

- This repository is a mixed workspace:
  - `apps/mobile`: Vue 3 + Vite + Capacitor mobile/web client
  - `packages/node-client`: TypeScript bridge used by the app to talk to the native plugin surface
  - `crates/reticulum_mobile`: Rust runtime, UniFFI bridge, and LXMF/Reticulum integration
  - `tools/codegen`: UniFFI binding generation scripts
  - `e2e`: Playwright end-to-end coverage
- Primary product focus is emergency coordination over Reticulum mesh networking, including peer discovery, action messages, event replication, and telemetry.

## Frontend Engineering

Read and apply [Frontend engineering principles](FRONTEND_ENGINEERING_PRINCIPLES.md)
before JavaScript/TypeScript UI design, implementation, refactoring, or review.
Its repository appendix gives the reviewed source boundaries and local checks.
Business/domain authority stays in the designated backend or native runtime;
stores, hooks, composables, and frontend services are not alternative owners.
This supplements the existing architecture, safety, toolchain, and workflow rules.

## Working Rules

- Start from the repo root unless a package-specific command clearly belongs elsewhere.
- Check `git status` before editing. This repo often has generated Android/Rust artifacts in the worktree.
- Do not hand-edit generated or build output unless the task is explicitly about generated artifacts or native packaging.
- Keep fixes scoped. A UI change should not casually rewrite transport or runtime behavior.
- Prefer updating the real source of truth rather than patching copied artifacts.
- Use the compiled `LXMF-rs` implementation through the existing Rust bridge and generated bindings. Do not recreate LXMF protocol functionality in TypeScript, Vue stores, or ad hoc Rust compatibility code when the compiled library already provides it.

## High-Value Directories

- `apps/mobile/src/views`: route-level screens
- `apps/mobile/src/components`: reusable UI pieces
- `apps/mobile/src/stores`: Pinia UI state, runtime projections, and request coordination
- `apps/mobile/src/utils`: presentation helpers and existing legacy adapters; do not extend protocol/domain authority here
- `apps/mobile/src/services`: platform-facing helpers such as sharing, notifications, telemetry helpers
- `apps/mobile/src/types/domain.ts`: shared app domain types
- `packages/node-client/src/index.ts`: TS client boundary for the native bridge
- `crates/reticulum_mobile/src/runtime.rs`: main Rust runtime behavior
- `crates/reticulum_mobile/src/sdk_bridge.rs`: SDK-facing LXMF bridge layer
- `crates/reticulum_mobile/src/jni_bridge.rs`: native boundary used by the mobile side
- `crates/reticulum_mobile/src/reticulum_mobile.udl`: UniFFI interface definition
- `docs/architecture.md`: transport and replication architecture notes

## Generated And Volatile Paths

Treat these as generated or disposable unless the task explicitly targets them:

- `node_modules/`
- `target/`
- `playwright-report/`
- `test-results/`
- `tmp/`
- `apps/tmp-playwright-ui.err`
- `apps/mobile/tmp-playwright-ui.err`
- `apps/mobile/tmp-playwright-ui.out`
- `apps/mobile/android/app/build/`
- `apps/mobile/android/app/src/main/jniLibs/`
- `apps/mobile/android/uniffi/`
- `apps/mobile/ios/uniffi/`

On Windows, broad recursive directory scans can fail inside Android build intermediates. Prefer scoped searches over targeted source directories instead of walking the entire repo.

## Expected App Conventions

- Vue code is written with Vue 3 Composition API and `<script setup lang="ts">`.
- TypeScript is `strict` in both the app and `packages/node-client`.
- Pinia stores own UI state, runtime projections, and request coordination. Business rules, authoritative transitions, replication/conflict decisions, and delivery semantics belong in the native runtime or designated service, not stores, composables, utilities, or templates.
- Reuse existing domain types from `apps/mobile/src/types/domain.ts` before inventing near-duplicates.
- Keep API/native payload translation behind `packages/node-client`; keep protocol semantics in the Rust runtime and compiled libraries. Trace callers of legacy `utils` codecs before migrating/removing them; do not add a second protocol implementation.
- App-wide button press feedback is defined on global `button` rules in `apps/mobile/src/styles.css`; component buttons should set the existing CSS custom properties rather than adding one-off `:active` behavior.
- Maintain the existing style conventions in touched files:
  - double quotes
  - semicolons
  - explicit typing when it improves clarity at boundaries

## Rust Skills Integration

Apply these additional rules whenever a task touches Rust code, `Cargo.toml`, or the UniFFI/native bridge:

- Treat the installed Rust skills bundle as the default routing layer for Rust work:
  - general Rust questions or ambiguous Rust tasks: `rust-router`
  - ownership, borrowing, lifetimes, and move errors: `m01-ownership`
  - smart pointers and resource ownership patterns: `m02-resource`
  - error modeling and propagation: `m06-error-handling`
  - async, `Send`/`Sync`, threading, and channels: `m07-concurrency`
  - `unsafe`, FFI, raw pointers, JNI, and bridge boundary reviews: `unsafe-checker`
- For new Rust crates or new `Cargo.toml` package sections created in this repo, default to:
  - `edition = "2024"`
  - `rust-version = "1.85"`
  - `[lints.rust] unsafe_code = "warn"`
  - `[lints.clippy] all = "warn"` and `pedantic = "warn"`
- Prefer domain-correct design fixes over borrow-checker workarounds. Do not reach for cloning or ownership duplication until the ownership model is justified by the runtime and protocol design.
- Use `?` and typed error propagation in library/runtime code instead of `unwrap()` or `expect()`, unless a crash is intentionally part of the boundary behavior.
- Every `unsafe` block must carry a nearby `// SAFETY:` comment that states the invariant making the block sound.
- Keep Rust changes aligned with the existing project architecture in this file, especially the rules about using the compiled `LXMF-rs` implementation through the current bridge instead of recreating protocol behavior in higher layers.

## Change Routing

Use this map to decide where a change belongs:

- UI layout, forms, route behavior:
  - `apps/mobile/src/views`
  - `apps/mobile/src/components`
- UI preferences, cached runtime projections, peer-list display, pending requests:
  - `apps/mobile/src/stores` and feature composables
- Presentation mapping and immediate input feedback:
  - focused helpers in `apps/mobile/src/utils`
- Authoritative message/event/telemetry workflows, mission sync/replication policy, wire formats, and announce semantics:
  - `crates/reticulum_mobile` and its compiled libraries, exposed through `packages/node-client`
- Capacitor-facing TypeScript API surface:
  - `packages/node-client/src/index.ts`
- Native runtime behavior, packet/LXMF handling, delivery tracking:
  - `crates/reticulum_mobile/src/runtime.rs`
  - `crates/reticulum_mobile/src/sdk_bridge.rs`
  - `crates/reticulum_mobile/src/jni_bridge.rs`
- UniFFI interface or generated mobile bindings:
  - `crates/reticulum_mobile/src/reticulum_mobile.udl`
  - then run the appropriate `tools/codegen` script instead of editing copied bindings by hand

## Build And Verification Commands

Run the narrowest command set that proves the change:

- Install JS dependencies:
  - `npm install`
- App development:
  - `npm run web:dev`
  - `npm run mobile:dev`
- Builds:
  - `npm run web:build`
  - `npm run mobile:build`
  - `npm run node-client:build`
  - `npm --workspace packages/node-client run build`
- Capacitor native workflow:
  - `npm --workspace apps/mobile run sync`
  - `npm --workspace apps/mobile run android`
  - `npm --workspace apps/mobile run ios`
  - Current project release work does not target iOS compilation. For Android packaging, prefer `npx cap sync android` from `apps/mobile` instead of full `npm --workspace apps/mobile run sync`, because full sync also tries the iOS CocoaPods step.
- Type checking:
  - `npm --workspace apps/mobile run typecheck`
- E2E:
  - `npx playwright install chromium`
  - `npm run test:e2e`
  - `npm run test:e2e:headed`
  - `npm run test:e2e:debug`
- Rust:
  - `cargo test --manifest-path crates/reticulum_mobile/Cargo.toml`
- UniFFI code generation:
  - PowerShell: `./tools/codegen/generate-uniffi-bindings.ps1 -Language kotlin`
  - PowerShell: `./tools/codegen/generate-uniffi-bindings.ps1 -Language swift`
  - Shell: `./tools/codegen/generate-uniffi-bindings.sh kotlin`
  - Shell: `./tools/codegen/generate-uniffi-bindings.sh swift`
  - The PowerShell script falls back to the workspace `tools/uniffi-bindgen` crate when `uniffi-bindgen` is not on `PATH`.
  - The shell script does not have the same fallback: it skips Kotlin binding generation without `uniffi-bindgen` on `PATH` and fails for Swift.
- Android release artifacts:
  - From `apps/mobile/android`: `cmd /c gradlew.bat assembleRelease bundleRelease`

On revisions containing the existing source-size/unit scripts, also use
`npm run check:source-size` and the relevant `npm run test:unit` /
`npm run test:node-client` checks. Preserve the 500-line source/class gate;
do not raise limits or compress code to pass it. Inspect the active revision
before assuming newer scripts exist. Documentation-only policy edits need
document/link/diff checks, not app/native builds or dependency installation.

There is no dedicated root lint script at the moment. For most app changes, `typecheck` + the relevant build + the closest Playwright spec is the minimum useful validation.

## Cross-Layer Change Rules

- If you change a payload shape or delivery flow in TypeScript, verify whether the same change must be reflected in:
  - `apps/mobile/src/utils/missionSync.ts`
  - `apps/mobile/src/utils/replicationParser.ts`
  - `packages/node-client/src/index.ts`
  - `crates/reticulum_mobile/src/jni_bridge.rs`
  - `crates/reticulum_mobile/src/runtime.rs`
  - `docs/architecture.md`
- Preserve the current architecture where LXMF behavior comes from compiled `LXMF-rs` code. Extend the bridge or SDK integration when needed, but do not duplicate encoding, delivery tracking, or protocol logic in higher layers just to bypass the compiled library.
- If you change the UniFFI contract, regenerate bindings instead of editing generated outputs manually.
- If you change event or telemetry behavior, update or add the closest Playwright coverage in `e2e/`.
- If transport behavior changes, document the new flow in `docs/architecture.md` or the relevant README.

## Environment Notes

- `crates/reticulum_mobile/Cargo.toml` currently points `lxmf` and `lxmf-sdk` to local path dependencies. Do not replace those paths casually; they reflect this workspace's current development setup.
- Android signing uses local, ignored configuration under `apps/mobile/android/keystore.properties`.
- This project currently does not try to compile for iOS. Do not treat iOS build or CocoaPods failures as release blockers unless the user explicitly asks for iOS work.
- Root Playwright config starts the web app and exercises the app through the browser at `/dashboard`.

## Definition Of Done

Before finishing, make sure you can state:

- what changed
- which layer(s) were touched
- which verification commands were run
- whether any generated artifacts were intentionally updated
- whether docs or tests were updated to match behavior changes
