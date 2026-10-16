import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./settingsSaveErrors.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const { describeSettingsSaveError } = await import(moduleUrl);

class FakeNodeError extends Error {
  constructor(message, code) {
    super(message);
    this.code = code;
  }
}

test("runtime household-name rejection names the household name field", () => {
  const failure = describeSettingsSaveError(new FakeNodeError(
    "household name is required (1 to 64 characters) when a household profile is set",
    "InvalidConfig",
  ));
  assert.equal(failure.field, "householdName");
  assert.equal(
    failure.message,
    "Settings not saved: household name is required (1 to 64 characters) when a household profile is set.",
  );
});

test("runtime household-ID and power rejections map to their fields", () => {
  assert.equal(
    describeSettingsSaveError(new FakeNodeError("household ID must be exactly 16 hexadecimal characters", "InvalidConfig")).field,
    "householdId",
  );
  assert.equal(
    describeSettingsSaveError(new FakeNodeError("power saver threshold must be 10, 20 or 30 percent", "InvalidConfig")).field,
    "powerThresholdPercent",
  );
  assert.equal(
    describeSettingsSaveError(new FakeNodeError("each role badge must be 1 to 24 characters", "InvalidConfig")).field,
    "roleBadges",
  );
});

test("a bare invalid config from an older runtime still tells the operator where to look", () => {
  const failure = describeSettingsSaveError(new FakeNodeError("invalid config", "InvalidConfig"));
  assert.equal(failure.field, undefined);
  assert.match(failure.message, /household profile/);
  assert.match(failure.message, /invalid config/);
});

test("unrelated failures pass their message through unchanged", () => {
  const failure = describeSettingsSaveError(new Error("Native app settings save verification failed."));
  assert.equal(failure.field, undefined);
  assert.equal(failure.message, "Native app settings save verification failed.");
  assert.equal(describeSettingsSaveError("socket closed").message, "socket closed");
});
