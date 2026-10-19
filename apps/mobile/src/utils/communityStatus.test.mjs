import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./communityStatus.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const { COMMUNITY_STATUS_OPTIONS, freshnessLabel, householdComposition, resolveHouseholdId } = await import(moduleUrl);

test("community status exposes the exact four operator actions", () => {
  assert.deepEqual(COMMUNITY_STATUS_OPTIONS.map(({ label }) => label), [
    "All Home", "Someone Missing", "Evacuated", "Needs Help",
  ]);
});

test("native-projected B04 records are presentation-only inputs", () => {
  const updatedAt = 1_710_000_000_000;
  const projection = {
    householdId: "0123456789abcdef", householdName: "Harbour House",
    adults: 2, children: 1, pets: 2, roleBadges: ["medic"],
    status: "needs_help", saverActive: true, updatedAtMs: updatedAt,
    sourceIdentity: "signed-source",
  };
  assert.equal(householdComposition(projection), "3 people · 2 pets");
  assert.equal(freshnessLabel(updatedAt, updatedAt + 3_600_000), "updated 1h ago");
});

test("a household profile without a typed ID falls back to the node's app destination", () => {
  const destination = "0123456789ABCDEF0123456789abcdef";
  assert.equal(resolveHouseholdId("", destination), "0123456789abcdef");
  assert.equal(resolveHouseholdId("   ", destination), "0123456789abcdef");
  assert.equal(resolveHouseholdId("FEDCBA9876543210", destination), "fedcba9876543210");
  assert.equal(resolveHouseholdId("", ""), "");
  assert.equal(resolveHouseholdId("", "not-a-destination"), "");
});

test("household status events are recognized by their uid", async () => {
  const { communityEventSourceIdentity } = await import(moduleUrl);
  assert.equal(
    communityEventSourceIdentity("rem-community-status-v1:E6FCF8F02E290ED46F88A729460DFFFC"),
    "e6fcf8f02e290ed46f88a729460dfffc",
  );
  assert.equal(communityEventSourceIdentity("evt-9e8e28fe"), "");
});

test("household status event details read as plain text", async () => {
  const { communityEventDetails } = await import(moduleUrl);
  assert.deepEqual(
    communityEventDetails({ householdName: "Eliopoli", adults: 2, children: 1, pets: 0, roleBadges: ["Medic"], saverActive: true }),
    ["Eliopoli", "3 people · 0 pets", "Medic", "Power saver on"],
  );
  assert.deepEqual(
    communityEventDetails({ householdName: " ", adults: 1, children: 0, pets: 1, roleBadges: [], saverActive: false }),
    ["1 person · 1 pet"],
  );
});

test("a beacon that was never published says so instead of claiming it is fresh", () => {
  assert.equal(freshnessLabel(0), "not published yet");
});

test("this node's own beacon is not counted as a nearby household", async () => {
  const { splitOwnCommunityRecord } = await import(moduleUrl);
  const own = { sourceIdentity: "E6FCF8F02E290ED46F88A729460DFFFC", status: "all_home" };
  const other = { sourceIdentity: "11111111111111111111111111111111", status: "one_missing" };
  assert.deepEqual(splitOwnCommunityRecord([own, other], "e6fcf8f02e290ed46f88a729460dfffc"), { own, others: [other] });
  assert.deepEqual(splitOwnCommunityRecord([other], ""), { own: undefined, others: [other] });
});

test("dashboard share time reads as plain words", async () => {
  const { sharedAgoLabel } = await import(moduleUrl);
  const now = 1_710_000_000_000;
  assert.equal(sharedAgoLabel(0, now), "Not shared yet");
  assert.equal(sharedAgoLabel(now - 20_000, now), "Shared just now");
  assert.equal(sharedAgoLabel(now - 30 * 60_000, now), "Shared 30m ago");
  assert.equal(sharedAgoLabel(now - 3 * 3_600_000, now), "Shared 3h ago");
});
