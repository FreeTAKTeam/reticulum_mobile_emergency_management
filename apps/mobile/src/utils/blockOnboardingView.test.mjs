import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./blockOnboardingView.ts", import.meta.url), "utf8");
const fixedVector = await readFile(
  new URL("../../android/app/src/test/resources/block-onboarding-max-v1.txt", import.meta.url),
  "utf8",
);
const transpiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const { completePeerTierMap, onboardingDestinations } = await import(moduleUrl);
const inspection = {
  issuerAppDestinationHex: "aa", issuerLxmfDestinationHex: "bb",
  trustedDestinationHashes: ["bb", "cc", "dd"],
};

test("Block review creates one complete unique destination-to-tier map", () => {
  assert.deepEqual(onboardingDestinations(inspection), ["aa", "bb", "cc", "dd"]);
  assert.deepEqual(completePeerTierMap(inspection, "inner", { cc: "inner" }), [
    { destinationHex: "aa", circleTier: "inner" },
    { destinationHex: "bb", circleTier: "outer" },
    { destinationHex: "cc", circleTier: "inner" },
    { destinationHex: "dd", circleTier: "outer" },
  ]);
});

test("additional trusted destinations default explicitly to Outer Circle", () => {
  const tiers = completePeerTierMap(inspection, "outer");
  assert.equal(tiers.length, onboardingDestinations(inspection).length);
  assert.equal(tiers.every(({ circleTier }) => circleTier === "outer"), true);
});

test("mobile treats the checked-in maximum signed vector as opaque text", () => {
  assert.equal(Buffer.byteLength(fixedVector, "utf8"), 1_999);
  assert.equal(fixedVector.startsWith("REMBC1:"), true);
  assert.equal(fixedVector.includes("{"), false);
});
