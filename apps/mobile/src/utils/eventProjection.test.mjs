import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

async function transpiledUrl(relativeUrl, replacements = new Map()) {
  const source = await readFile(new URL(relativeUrl, import.meta.url), "utf8");
  let output = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  for (const [specifier, replacement] of replacements) {
    output = output.replaceAll(`"${specifier}"`, `"${replacement}"`);
  }
  return `data:text/javascript;base64,${Buffer.from(output).toString("base64")}`;
}

const mecpUrl = await transpiledUrl("./mecp.ts");
const r3aktUrl = await transpiledUrl("./r3akt.ts");
const eventUrl = await transpiledUrl("./eventProjection.ts", new Map([
  ["./mecp", mecpUrl],
  ["./r3akt", r3aktUrl],
]));
const {
  encodeEventTypeKeywords,
  getEventUpdatedAt,
  normalizeEvent,
  toTimelineRecord,
} = await import(eventUrl);

test("legacy event shapes normalize into the native projection contract", () => {
  const event = normalizeEvent({
    uid: " event-1 ",
    type: "Medical",
    summary: " Field report ",
    callsign: " Alpha ",
    sourceIdentity: " identity ",
    updatedAt: 1_700_000_000,
    keywords: ["duplicate", "duplicate"],
  });

  assert.equal(event.args.entry_uid, "event-1");
  assert.equal(event.args.mission_uid, "r3akt-default-mission");
  assert.equal(event.args.content, "Field report");
  assert.equal(event.args.callsign, "Alpha");
  assert.equal(event.source.rns_identity, "identity");
  assert.deepEqual(event.args.keywords, ["duplicate", "r3akt:event-type:Medical"]);
  assert.equal(getEventUpdatedAt(event), 1_700_000_000_000);
});

test("timeline projection retains plain event summaries and type tags", () => {
  const event = normalizeEvent({
    uid: "event-2",
    type: "Logistics",
    summary: "Supply cache established",
    callsign: "Bravo",
    updatedAt: 1_700_000_000_000,
  });

  assert.deepEqual(toTimelineRecord(event), {
    uid: "event-2",
    type: "Logistics",
    summary: "Supply cache established",
    callsign: "Bravo",
    updatedAt: 1_700_000_000_000,
    mecp: undefined,
  });
});

test("event type keywords replace stale tags and remove duplicates", () => {
  assert.deepEqual(
    encodeEventTypeKeywords("Medical", ["ops", "ops", "r3akt:event-type:Old"]),
    ["ops", "r3akt:event-type:Medical"],
  );
});
