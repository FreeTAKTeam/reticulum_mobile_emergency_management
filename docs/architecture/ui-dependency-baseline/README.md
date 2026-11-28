# REM UI dependency baseline and module boundary policy

Verified baseline for the Vue/TypeScript client (#386) and the first cycle fix (#387).
Source assessment: FreeTAKTeam/R3AKT `docs/architecture/assessment-2026-11-28` (R3AKT#12).
Report-only: nothing here gates CI.

## Contents

| Path | What |
| --- | --- |
| `rem-ui.policy.json` | Reviewed module policy (`archgraph/policy/v1`) |
| `before/` | Pinned baseline, revision `8bcfdd3d4a656b70c4ef958f621a3f0a24ccca0f` (main, 2026-11-27, clean tree) |
| `after/` | Same analysis after #387 (commit recorded in `validation.json` → `metadata.source.commit`) |
| `after-388/` | Same analysis after #388, with the stricter policy |
| `*/dsm.csv`, `*/dsm-reordered.csv`, `*/components.csv`, `*/groups.csv` | archgraph `analyze` output: DSM sorted by id, DSM providers-first with cycles as blocks, fan-in/out, per-layer cohesion/coupling with denominators |
| `*/validation.json` | archgraph `validate` report: findings with first source evidence, every runtime edge, input SHA-256 |
| `../../../tools/architecture/ts-import-graph.mjs` | Parser-backed extractor; fixtures in `ts-import-graph.test.mjs` (`npm run test:architecture`) |
| `../../../tools/architecture/ui-baseline.sh` | Reproduces one directory |
| `../../testing/frontend-regression.md` | Test toolchain, commands and clean-checkout results per architecture PR (#389) |

## Reproduce

Needs [archgraph](https://github.com/FreeTAKTeam/archgraph) 0.1.0 and `npm install` (the
extractor uses the repo's `typescript` and `@vue/compiler-sfc`).

```bash
git worktree add --detach /tmp/rem-pin 8bcfdd3d4a656b70c4ef958f621a3f0a24ccca0f
ARCHGRAPH=archgraph tools/architecture/ui-baseline.sh /tmp/rem-before /tmp/rem-pin
ARCHGRAPH=archgraph tools/architecture/ui-baseline.sh /tmp/rem-after .
```

`--today 2026-11-28` is fixed in the script so exception expiry is deterministic.

## Extraction

- **Parser:** each `.ts/.js/.mjs` file, and both `<script>` blocks of each `.vue` file
  (`@vue/compiler-sfc`), is parsed with the TypeScript compiler API. Imports inside comments,
  strings and templates are ignored by construction.
- **Edge kinds:** `import` (runtime, including side-effect imports and `export … from`),
  `type-import` (`import type`, `export type`, named bindings that are all `type`, and
  `import("x").T` type nodes), `dynamic-import` (`import("literal")`). Files matching
  `*.test.*`/`*.spec.*` and the `e2e` tree give `test-*` kinds, which are outside the policy
  scope.
- **Aliases:** `@reticulum/node-client` → `packages/node-client/src/index.ts` (the package's
  `exports`); `/src` → `apps/mobile/src` (Vite root-absolute paths used by e2e specs).
- **Granularity:** a component is a source directory plus its first sub-directory
  (`apps/mobile/src/components`); files directly in `apps/mobile/src` form the app shell
  component. `components/settings` is therefore part of `components`.

### Cross-check against the regex extractor

At the pinned revision the parser reproduces the assessment exactly at the pair level:
**11 components, 34 runtime pairs, density 34/110 = 0.309, 2 cycles, 3 layer warnings**.
Edge-kind labels differ on three cells, all regex misreadings:

| Cell | Regex | Parser | Cause |
| --- | --- | --- | --- |
| `stores → types` | `dynamic-import+type-import` | `type-import` | `import("../types/domain").PeerConnectionState` in a type position (`stores/nodeLifecycleController.ts:64`) read as a dynamic import |
| `stores → node-client` | `dynamic-import+import+type-import` | `import+type-import` | same, `stores/nodeProjectionController.ts:62` |
| `composables → utils` | `import` | `import+type-import` | `import { type A, type B } from` (`composables/useChecklistList.ts:7`) not recognised as type-only |

No imports in comments or strings changed a pair.

### Denominators and counts (pinned revision)

- 292 source files in scope, 24 e2e files; 526 runtime import statements across 34 pairs;
  8 further test-only pairs (40 with tests).
- 308 intra-component imports and 18 asset imports are not modelled; 413 third-party import
  statements to 24 packages are not modelled (listed in `validation.json`).
- 53 unresolved, all `test-dynamic-import` with a computed specifier (`import(moduleUrl)` data
  URLs in `utils/*.test.mjs`, `import(/* @vite-ignore */ path)` in e2e). No runtime import is
  unresolved.

### Known limitations

- `require()`, CSS `@import`, and `<style>`/`<template>` asset references are not read.
- Edges are direct; transitive reachability is not checked.
- Granularity is directory-level; cycles between files inside one directory are not visible.
- archgraph allow rules match a pair if *any* of its kinds match, so the policy cannot allow
  type-only edges on a pair that also carries runtime edges. Type-only edges are therefore
  held to the same layer order as runtime edges (see policy).

## Cycles at the pinned revision

1. **`apps/mobile/src` ↔ `components` ↔ `composables` ↔ `views`: confirmed.** The back-edges
   that close it:
   - `components → apps/mobile/src`: `components/AppToolsMenu.vue:5` imports `../appNavigation`.
   - `composables → components`: `composables/useSetupWizard.ts:4,11,12` import
     `components/settings/{settingsFormSlices,settingsPageCopy,sosFormDraft}`.

   With `apps/mobile/src → components` (`App.vue:9`) and `views → composables`, these form
   the strongly connected set.
2. **`services` ↔ `utils`: confirmed.** `services/hubRegistryBootstrap.ts:4` imports
   `utils/missionSync`; `utils/checklistNotifications.ts:3` imports
   `services/operationalNotifications`. Left for #388 under a dated exception.

Both are real at the policy's directory granularity, so neither is an extraction artifact.
Neither is a file-level import loop, though. The file-level graph
(`ui-baseline.sh` → `files/`, `--granularity file`) has **316 files, 788 pairs and 0 cycles**
at the pinned revision. Each cycle comes from module placement (ownership), not from code
that imports itself back.

## Module policy

Reviewed from `AGENTS.md` and `FRONTEND_ENGINEERING_PRINCIPLES.md`, not fitted to the graph.

Layers, lowest first. A module may depend on its own layer or lower ones:

| Layer | Members | Owns |
| --- | --- | --- |
| `types` | `apps/mobile/src/types` | Shared domain types; depends on nothing in the app (`types-are-leaf`) |
| `native-client` | `packages/node-client/src` | Bridge client; never imports the app (`node-client-is-app-independent`) |
| `utils` | `apps/mobile/src/utils` | Reusable pure helpers: presentation mapping, input normalisation, settings draft slices and page copy |
| `services` | `apps/mobile/src/services` | Platform-facing helpers (sharing, notifications, storage) |
| `state` | `apps/mobile/src/stores`, `apps/mobile/src/composables` | Pinia stores, runtime projections, reusable composables, store-backed form helpers (`sosFormDraft`) |
| `presentation` | `apps/mobile/src/components`, `apps/mobile/src/views` | Generic and feature components (including `components/settings` and its feature-local `use*` helpers), menu/navigation metadata (`components/appNavigation.ts`), route screens |
| `app-shell` | `apps/mobile/src` | `App.vue`, `main.ts`, `router.ts`, `settingsRoutes.ts` |

Rules:

- **Layer order** (error since #388; warning before): an upward edge is a finding. Type-only edges count: an
  upward type import still couples a lower module to a higher module's contract. Move the
  type down instead (as `NodeSettingsSlice` moved into `utils/settingsFormSlices.ts`).
- **`views-only-from-router`** (error): route screens are imported only by the app shell
  (the router) and by other views.
- **Cycles** (error) in runtime and type-only edges.
- **Test code** (`*.test.*`, `*.spec.*`, `e2e/`) may import any layer; its `test-*` edges are
  outside the scope and never create findings.

Exceptions (owner REM maintainers, expire 2027-02-28): the `services` ↔ `utils` cycle and
the `utils → services` layer edge. Both were deferred by #388 as operational (see below).

### Module ownership

| Concern | Owner | Notes |
| --- | --- | --- |
| Native records and bridge types (`RnodeBleDeviceRecord`, `SosSettingsRecord`, …) | `@reticulum/node-client` | Import them from the package, not through a `services/*` re-export |
| Shared app domain types | `apps/mobile/src/types/domain.ts` | Reuse before adding near-duplicates |
| Reusable UI primitives | top-level `components/*.vue` (`StatusPill`, `ListWindowControls`, `PowerSaverBadge`, `SplashScreen`) | Feature components live in `components/<feature>/` |
| Menu and navigation metadata | `components/appNavigation.ts` | Read by `App.vue` and `AppToolsMenu` |
| Formatting and presentation helpers | pure modules in `utils/` (`listWindow`, `settingsIndexSummaries`, `mapMarkerPresentation`, …) | No imports from `services`, `stores`, `composables`, `components`, `views` (except the deferred operational edges below) |
| Generic settings/configuration interfaces | `utils/settingsFormSlices.ts` (`NodeSettingsSlice`, draft slices), `utils/settingsPageCopy.ts`, `stores/nodeSettingsModel.ts` | Store-backed form helpers go in `composables/` (`sosFormDraft`) |
| Reusable state helpers | `composables/` (`useListWindow`, …) | Feature-local `use*` helpers may stay beside their components |

## Results

| | Pairs | Density | Cycles | Active findings | Suppressed |
| --- | --- | --- | --- | --- | --- |
| Pinned `8bcfdd3` | 34 | 34/110 = 0.309 | 2 (4, 2) | 1 error (cycle 1), 2 warnings | 2 (#388) |
| After #387 | 32 | 32/110 = 0.291 | 1 (2) | 0 | 2 (#388) |
| After #388 | 32 | 32/110 = 0.291 | 1 (2) | 0 | 2 (deferred, operational) |

#387 removed `components → apps/mobile/src` and `composables → components` by moving four
files, without changing behaviour:

| From | To | Why |
| --- | --- | --- |
| `appNavigation.ts` | `components/appNavigation.ts` | Menu metadata used by `AppToolsMenu` and `App.vue`; beside its menu component |
| `components/settings/settingsFormSlices.ts` | `utils/settingsFormSlices.ts` | Pure draft↔settings mapping; now also declares `NodeSettingsSlice` |
| `components/settings/settingsPageCopy.ts` | `utils/settingsPageCopy.ts` | Static copy shared by Settings and the setup wizard |
| `components/settings/sosFormDraft.ts` | `composables/sosFormDraft.ts` | Depends on `stores/sosStore`, so it cannot sit in `utils` |

Focused tests for the moved helpers: `apps/mobile/src/utils/settingsFormSlices.test.ts`.

### #388: remaining findings

After #387, archgraph reports one cycle, `services` ↔ `utils`, plus its `utils → services`
layer edge. At file level there are no cycles. Each edge, classified:

| Edge | Evidence | Class | Action |
| --- | --- | --- | --- |
| `utils → services` (type) | `utils/rnodeUsbPairing.ts:1` imported `RnodeBleDeviceRecord` via `services/rnodeBluetooth`, which only re-exports it from `@reticulum/node-client` | Type-only, ownership artifact | **Fixed:** imports the type from `@reticulum/node-client` |
| `utils → services` | `utils/checklistNotifications.ts:3` → `services/operationalNotifications` | Runtime, operational (checklist notifications) | Deferred |
| `utils → services` | `utils/eamProjection.ts:1`, `utils/eventProjection.ts:1`, `utils/legacyStorage.ts:4` → `services/operationalStorage` | Runtime, operational (EAM, events, legacy storage) | Deferred |
| `services → utils` | `services/hubRegistryBootstrap.ts:4,11` → `utils/missionSync`, `utils/r3akt` | Runtime, operational (mission sync, hub registry) | Deferred |

No generic UI cycle remains, at directory or file level. The remaining cycle runs only
through operational modules. Breaking it means first deciding who owns mission sync and
checklist notifications (assessment proposal 4). That decision is out of scope for #388, so
the cycle stays under its dated exception, tracked by #393.

The policy now makes `layer-order` an error, so any new upward edge outside the exception
is an error finding. Validated with `--blocking --baseline after/validation.json`: exit 0,
0 new findings.

