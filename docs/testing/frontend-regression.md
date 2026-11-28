# Frontend regression tests for architecture changes

Closes the R3AKT pilot's test-evidence gap (#389): the REM UI suite was not run there because
the exported copy had no `node_modules`.

## Toolchain

| Item | Value |
| --- | --- |
| Package manager | npm workspaces (`apps/*`, `packages/*`) |
| Lockfile | `package-lock.json`, lockfileVersion 3; install with `npm ci` only |
| Node in CI | 20 (`.github/workflows/rust.yml`, `plugins.yml`) |
| Node used for the evidence below | v24.18.0, npm 11.16.0 |
| Test runners | `node --test` (utils `*.test.mjs`, tools), vitest 4.1.12, Playwright 1.58.2 (Chromium, `chromium` and `mobile-chromium` projects) |

## Commands

```bash
npm ci
npm run test:unit                          # all unit suites below
npm --workspace apps/mobile run typecheck
npm run web:build
npm run check:source-size
npx playwright install chromium && npm run test:e2e   # optional, operational e2e
```

`test:unit` runs, in order:

| Script | Covers |
| --- | --- |
| `test:source-size` | source-size gate helpers |
| `test:architecture` | the import-graph extractor fixtures |
| `test:app-unit` | `apps/mobile/src/utils/*.test.mjs`, vitest `services/` and `utils/settingsFormSlices.test.ts`, then `test:components` |
| `test:components` | `apps/mobile/src/components/**/*.test.ts`, rendered with `vue/server-renderer` under the app's Vite config |
| `test:node-client` | `packages/node-client` vitest |
| `test:dependency-compatibility` | dependency compatibility checks |

### Generic UI tests added by #389

Synthetic props only. No native bridge, no stores, no network.

| File | Checks |
| --- | --- |
| `components/appShell.test.ts` | every footer and menu entry resolves to a route in `router.ts`; every navigation icon has paths; `AppToolsMenu` renders one tile per entry, marks exactly one active, and renders nothing when closed |
| `components/sharedUi.test.ts` | `StatusPill` value, meaning and colour/density classes; `ListWindowControls` only over 200 items, range text, disabled state |
| `components/settings/settingsPresentation.test.ts` | `SettingsSaveBar` clean/dirty/saving/feedback states; `SettingsLeaveDialog` open/closed alert dialog; `SettingsPermissionRow` initial state |

A mutation check confirmed they can fail: breaking the active-tab test in `AppToolsMenu` or
renaming a route made two tests fail.

### Clean-checkout runner

`tools/architecture/frontend-regression.sh <commit> <work-dir> [--e2e]` checks out the commit
in a temporary worktree, runs `npm ci` and every step above, and writes:
- `results.tsv`: step, exit code, seconds;
- `environment.tsv`: commit, Node, npm, lockfile SHA-256;
- `tool-versions.txt`;
- one log per step.

## Results at pinned commits (2026-11-28)

Lockfile SHA-256 `8424ab1cf36f8dc919377991581adb46826ad4bc0ed6c65d0498cb780363f079` at every
commit (unchanged).

| Commit | What | npm ci | unit | typecheck | web build | source size | e2e |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `8bcfdd3` | Baseline (assessment pin, `main`) | 0 | 0 | 0 | 0 | 0 | 0: 164 passed |
| `177ee3f` | #391 (#386, #387) | 0 | 0 | 0 | 0 | 0 | not run |
| `0ed815a` | #392 (#388) | 0 | 0 | 0 | 0 | 0 | not run |
| `230f413` | #389 tests | 0 | 0 | 0 | 0 | 0 | 0: 164 passed |

Values are exit codes. Unit test counts:

| Suite | `8bcfdd3` | `177ee3f` | `0ed815a` | `230f413` |
| --- | --- | --- | --- | --- |
| source-size (node) | 7 | 7 | 7 | 7 |
| architecture (node) | — | 4 | 4 | 4 |
| utils (node) | 215 pass, 25 todo | 215 pass, 25 todo | 215 pass, 25 todo | 215 pass, 25 todo |
| services (+ settings slices) vitest | 9 | 14 | 14 | 14 |
| components vitest | — | — | — | 10 |
| node-client vitest | 53 | 53 | 53 | 53 |
| dependency-compatibility (node) | 5 | 5 | 5 | 5 |

No failures and no skipped tests at any commit. No new failure is attributable to the
architecture changes.

### Known environmental items

- **25 `todo` tests:** MECP conformance vectors in `utils/mecp.conformance.test.mjs`, marked
  `# TODO`. node:test prints them under "failing tests", but they do not fail the run.
- **Nested worktrees:** vitest path filters are substring matches. In a checkout that holds
  nested worktrees (for example `.claude/worktrees/*`), the old
  `vitest run apps/mobile/src/services` also ran the nested copies (12–13 files instead of
  3–4). `test:app-unit` now passes `--dir apps/mobile/src` so it scans only this tree.
- **Flaky e2e test:** `e2e/settings.spec.ts:315` ("semi-autonomous RCH exposes effective
  connected routing state") fails intermittently on unchanged `main` too (fails on
  `--repeat-each=2`). It passed in both full runs above.
- **npm 11 install scripts:** npm 11 lists install scripts it does not run (`allow-scripts`),
  for example `esbuild` and `msgpackr-extract`. Builds and tests pass without them. CI uses
  Node 20 / npm 10, which runs them.
