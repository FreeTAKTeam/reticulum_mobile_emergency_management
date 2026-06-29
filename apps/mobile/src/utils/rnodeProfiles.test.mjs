import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./rnodeProfiles.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const {
  RNODE_REGION_SPECS,
  normalizeRnodeSettings,
  resolveRnodeFrequencyForRegionChange,
  rnodeRegionDefaultFrequencyHz,
} = await import(moduleUrl);

test("all supported RNode regions are available to app configuration", () => {
  assert.deepEqual(
    RNODE_REGION_SPECS.map((region) => region.id),
    ["US915", "EU868", "AU915", "AS923", "IN865", "KR920", "RU864"],
  );
});

test("region defaults resolve to exact LoRa center frequencies", () => {
  assert.equal(rnodeRegionDefaultFrequencyHz("US915"), 915_000_000);
  assert.equal(rnodeRegionDefaultFrequencyHz("EU868"), 868_000_000);
  assert.equal(rnodeRegionDefaultFrequencyHz("AS923"), 923_000_000);
  assert.equal(rnodeRegionDefaultFrequencyHz("IN865"), 865_000_000);
});

test("normalized settings add region default frequency when missing", () => {
  assert.equal(normalizeRnodeSettings({ region: "AS923" }).frequencyHz, 923_000_000);
});

test("normalized settings preserve exact configured frequency", () => {
  assert.equal(
    normalizeRnodeSettings({
      region: "EU868",
      frequencyHz: 869_525_000,
    }).frequencyHz,
    869_525_000,
  );
});

test("region change replaces frequency only when it still matches the previous default", () => {
  assert.equal(resolveRnodeFrequencyForRegionChange("EU868", "AS923", 868_000_000), 923_000_000);
  assert.equal(resolveRnodeFrequencyForRegionChange("EU868", "AS923", 869_525_000), 869_525_000);
});
