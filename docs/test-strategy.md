# Independent verification evidence

The CI workflow keeps focused unit tests and integration/browser evidence, with
no mass deletion and no relaxed quality or security rules.

- Web matrix: source-size enforcement, typecheck, source-size regression tests,
  app unit tests, node-client tests, node-client build, web build and mobile build.
- npm dependency audit: independent job, same moderate severity threshold.
- Rust matrix against the immutable LXMF release: format, clippy, runtime tests,
  audit, with JNI macro clippy retained. The standalone macro library has no committed
  lockfile; the runtime still uses its committed lockfile.
- Playwright: independent full desktop/mobile Chromium matrix; upload reports
  and traces after failure.
- Current LXMF main compatibility: scheduled/manual job generating an ephemeral
  lockfile, with independent clippy and runtime-test matrix jobs.

Formatting selects the REM package rather than the sibling LXMF workspace.
Both matrices disable fail-fast. Audit, clippy, typecheck or build failures cannot
cancel unrelated test jobs. A failed lane still fails the workflow; no
continue-on-error or new advisory exceptions were introduced.

Run the matching local commands separately to preserve every result:

```sh
npm run check:source-size
npm --workspace apps/mobile run typecheck
npm run test:source-size
npm run test:app-unit
npm run test:node-client
npm run node-client:build
npm run web:build
npm run mobile:build
npm audit --audit-level=moderate
cargo fmt --manifest-path crates/reticulum_mobile/Cargo.toml -- --check
cargo clippy --manifest-path crates/reticulum_mobile/Cargo.toml --locked --all-targets -- -D warnings
cargo test --manifest-path crates/reticulum_mobile/Cargo.toml --locked
cargo audit --file crates/reticulum_mobile/Cargo.lock
npm run test:e2e
```

Native Android instrumentation remains separate; browser tests do not establish
JNI/device or physical mesh acceptance. Local path dependencies use the existing
sibling LXMF checkout: a dirty/current sibling is not the pinned hosted release.
Do not rewrite that checkout or silently update committed locks to manufacture
compatibility. Report that mismatch explicitly when locked local checks block.
