import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import { ARCHGRAPH_VERSION, runGate } from "./ui-gate.mjs";

const archgraph = process.env.ARCHGRAPH || "archgraph";
const available = spawnSync(archgraph, ["--version"], { encoding: "utf8" }).stdout?.trim() === ARCHGRAPH_VERSION;
const skip = available ? false : `${ARCHGRAPH_VERSION} not on PATH or in $ARCHGRAPH`;
const today = "2026-11-28";

function tree(files) {
  const root = mkdtempSync(path.join(tmpdir(), "ui-gate-"));
  for (const [rel, text] of Object.entries(files)) {
    mkdirSync(path.dirname(path.join(root, rel)), { recursive: true });
    writeFileSync(path.join(root, rel), text);
  }
  return root;
}

// A small app that obeys the policy: views -> stores -> utils -> types.
const clean = {
  "packages/node-client/src/index.ts": "export type Rec = { id: string };\n",
  "apps/mobile/src/main.ts": "import \"./views/Home.vue\";\n",
  "apps/mobile/src/types/domain.ts": "export type Status = \"Green\";\n",
  "apps/mobile/src/utils/format.ts": "import type { Status } from \"../types/domain\";\nexport const f = (s: Status) => s;\n",
  "apps/mobile/src/stores/app.ts": "import { f } from \"../utils/format\";\nexport const s = f;\n",
  "apps/mobile/src/views/Home.vue": "<script setup lang=\"ts\">\nimport { s } from \"../stores/app\";\n</script>\n",
};

test("a clean synthetic tree passes and matches its own baseline", { skip }, () => {
  const root = tree(clean);
  const first = runGate({ root, out: path.join(root, "out0"), archgraph, today, baseline: null });
  assert.equal(first.code, 0);
  const again = runGate({ root, out: path.join(root, "out1"), archgraph, today, baseline: path.join(root, "out0/validation.json") });
  assert.equal(again.code, 0);
  assert.equal(again.report.counts.new, 0);
  assert.match(again.summary, /## New findings since baseline\n- none/);
});

test("a new cycle and a forbidden import are reported, without failing in report-only mode", { skip }, () => {
  const root = tree(clean);
  runGate({ root, out: path.join(root, "base"), archgraph, today, baseline: null });
  const baseline = path.join(root, "base/validation.json");
  writeFileSync(path.join(root, "apps/mobile/src/types/domain.ts"),
    "import { f } from \"../utils/format\";\nexport type Status = \"Green\";\nexport const g = f;\n");
  writeFileSync(path.join(root, "apps/mobile/src/utils/format.ts"),
    "import { s } from \"../stores/app\";\nexport const f = (x: string) => x;\nexport const t = s;\n");

  const report = runGate({ root, out: path.join(root, "r"), archgraph, today, baseline });
  assert.equal(report.code, 0);
  const fresh = report.report.findings.filter((f) => f.baseline_state === "new").map((f) => f.rule);
  assert.ok(fresh.includes("cycle"), fresh.join());
  assert.ok(fresh.includes("types-are-leaf"), fresh.join());
  assert.match(report.summary, /\*\*types-are-leaf\*\*: `apps\/mobile\/src\/types` → `apps\/mobile\/src\/utils` \(`apps\/mobile\/src\/types\/domain.ts:1`\)/);
  assert.match(report.summary, /New cycles:/);

  const blocking = runGate({ root, out: path.join(root, "b"), archgraph, today, baseline, blocking: true });
  assert.equal(blocking.code, 1);
});

test("a broken gate exits 2 instead of reporting clean", () => {
  const root = tree(clean);
  const result = runGate({ root, out: path.join(root, "x"), archgraph: path.join(root, "missing-archgraph"), today });
  assert.equal(result.code, 2);
  assert.match(result.summary, /broken/);
});

test("an invalid policy exits 2", { skip }, () => {
  const root = tree({ ...clean, "bad.policy.json": "{\"schema\": \"nope\"}\n" });
  const result = runGate({ root, out: path.join(root, "x"), archgraph, today, policy: path.join(root, "bad.policy.json") });
  assert.equal(result.code, 2);
});
