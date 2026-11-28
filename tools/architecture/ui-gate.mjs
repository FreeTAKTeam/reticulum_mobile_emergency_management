// Report-only REM UI architecture gate (#390).
// Extracts the UI import graph, validates it with pinned archgraph against the committed
// baseline and policy, and writes graph.json, validation.json, validation.sarif and summary.md.
//
//   node tools/architecture/ui-gate.mjs --out <dir> [--root .] [--blocking] [--today YYYY-MM-DD]
//
// Exit 0 in report-only mode whatever the findings are. Exit 2 when the gate itself is
// broken (wrong archgraph version, bad policy or graph), so a broken gate never reads as
// clean. With --blocking, exit 1 on new active error findings: enable it only after the
// owners approve the policy (docs/architecture/ui-dependency-baseline/README.md).
import { spawnSync } from "node:child_process";
import { appendFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

import { buildGraph } from "./ts-import-graph.mjs";

export const ARCHGRAPH_VERSION = "archgraph 0.1.0";
const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const DOCS = path.join(REPO, "docs/architecture/ui-dependency-baseline");
export const GRAPH_SETTINGS = {
  sources: ["apps/mobile/src", "packages/node-client/src"],
  testSources: ["e2e"],
  aliases: { "@reticulum/node-client": "packages/node-client/src/index.ts", "/src": "apps/mobile/src" },
};

const list = (items, render) => (items.length ? items.map((x) => `- ${render(x)}`).join("\n") : "- none");
const where = (f) => (f.evidence?.[0] ? ` (\`${f.evidence[0].location}:${f.evidence[0].line}\`)` : "");
const subject = (f) => (f.components ? f.components.join(" ↔ ") : `\`${f.from}\` → \`${f.to}\``);

export function summarize(report, graph, mode) {
  const active = report.findings.filter((f) => !f.suppressed_by);
  const suppressed = report.findings.filter((f) => f.suppressed_by);
  const fresh = active.filter((f) => f.baseline_state === "new");
  const forbidden = active.filter((f) => !["cycle", "layer-order"].includes(f.rule));
  const runtimeUnresolved = graph.unresolved.filter((u) => !u.kind.startsWith("test-"));
  const b = report.baseline ?? {};
  const src = report.metadata.source;
  return [
    `# REM UI architecture gate (${mode})`,
    "",
    `- Tool: ${report.metadata.tool} ${report.metadata.tool_version}; extractor: ${src.extractor}`,
    `- Commit: \`${src.commit ?? "unknown"}\`${src.dirty ? " (dirty)" : ""}; policy sha256 \`${report.metadata.settings.policy_sha256}\``,
    `- Outcome: **${report.outcome}**. Errors ${report.counts.error}, warnings ${report.counts.warning}, notes ${report.counts.note}, suppressed ${report.counts.suppressed}, new ${report.counts.new}`,
    "",
    "## New findings since baseline",
    list(fresh, (f) => `**${f.rule}** (${f.severity}): ${subject(f)}${where(f)}`),
    "",
    "## Cyclic groups",
    list(report.cycles, (c) => c.join(" ↔ ")),
    ...(b.new_cycles?.length ? ["", "New cycles:", list(b.new_cycles, (c) => c.join(" ↔ "))] : []),
    "",
    "## Forbidden imports (policy rules)",
    list(forbidden, (f) => `**${f.rule}**: ${subject(f)}${where(f)}`),
    "",
    "## Layer-order findings",
    list(active.filter((f) => f.rule === "layer-order"), (f) => `${subject(f)}${where(f)}`),
    "",
    "## Documented exceptions",
    list(suppressed, (f) => `${f.rule}: ${subject(f)}. Owner ${f.suppressed_by.owner}, expires ${f.suppressed_by.expires}`),
    "",
    "## Unresolved imports",
    `- Runtime: ${runtimeUnresolved.length}; test-only: ${graph.unresolved.length - runtimeUnresolved.length}`,
    ...runtimeUnresolved.map((u) => `  - \`${u.target}\`: ${u.reason}${where(u)}`),
    "",
    "## Baseline comparison",
    `- New edges: ${b.new_edges?.length ?? 0}${(b.new_edges ?? []).map(([f, t]) => `\n  - \`${f}\` → \`${t}\``).join("")}`,
    `- Removed edges: ${b.removed_edges?.length ?? 0}${(b.removed_edges ?? []).map(([f, t]) => `\n  - \`${f}\` → \`${t}\``).join("")}`,
    `- Resolved findings: ${b.resolved_findings?.length ?? 0}${(b.resolved_findings ?? []).map((r) => `\n  - \`${r}\``).join("")}`,
    "",
  ].join("\n");
}

export function runGate({
  root = REPO, out, archgraph = process.env.ARCHGRAPH || "archgraph",
  policy = path.join(DOCS, "rem-ui.policy.json"),
  baseline = path.join(DOCS, "after-388/validation.json"),
  blocking = false, today = new Date().toISOString().slice(0, 10), settings = GRAPH_SETTINGS,
}) {
  mkdirSync(out, { recursive: true });
  const fail = (message) => ({ code: 2, summary: `# REM UI architecture gate: broken\n\n${message}\n` });
  const version = spawnSync(archgraph, ["--version"], { encoding: "utf8" });
  if (version.status !== 0 || version.stdout.trim() !== ARCHGRAPH_VERSION) {
    return fail(`Expected \`${ARCHGRAPH_VERSION}\`, got \`${(version.stdout || version.error?.message || "").trim()}\`.`);
  }
  const graph = buildGraph({ root, ...settings });
  const files = { graph: path.join(out, "graph.json"), report: path.join(out, "validation.json"), sarif: path.join(out, "validation.sarif") };
  writeFileSync(files.graph, JSON.stringify(graph, null, 2) + "\n");
  const args = ["validate", "--graph", files.graph, "--policy", policy, ...(baseline ? ["--baseline", baseline] : []),
    "--today", today, "--out", files.report, "--sarif", files.sarif, ...(blocking ? ["--blocking"] : [])];
  const run = spawnSync(archgraph, args, { encoding: "utf8" });
  if (run.status !== 0 && run.status !== 1) return fail(`archgraph validate failed (${run.status}):\n\n${run.stderr}`);
  const report = JSON.parse(readFileSync(files.report, "utf8"));
  const summary = summarize(report, graph, blocking ? "blocking" : "report-only");
  writeFileSync(path.join(out, "summary.md"), summary);
  return { code: blocking ? run.status : 0, summary, report };
}

function main() {
  const { values } = parseArgs({
    options: {
      out: { type: "string" }, root: { type: "string" }, policy: { type: "string" },
      baseline: { type: "string" }, today: { type: "string" }, blocking: { type: "boolean", default: false },
    },
  });
  if (!values.out) throw new Error("--out is required");
  const opts = Object.fromEntries(Object.entries(values).filter(([, v]) => v !== undefined));
  const { code, summary } = runGate({ ...opts, out: path.resolve(values.out) });
  process.stdout.write(summary);
  if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, summary);
  process.exitCode = code;
}

if (import.meta.url === `file://${process.argv[1]}`) main();
