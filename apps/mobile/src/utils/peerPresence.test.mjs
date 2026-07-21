import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./peerPresence.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const { peerHasFreshPresence, peerPresenceFreshnessMs } = await import(moduleUrl);

test("default presence remains fresh through the announce interval and grace", () => {
  assert.equal(peerPresenceFreshnessMs(1800, 30), 31 * 60_000);
  assert.equal(peerHasFreshPresence({
    activeLink: false,
    lastSeenAt: 0,
    nowMs: 30 * 60_000,
    announceIntervalSeconds: 1800,
    staleAfterMinutes: 30,
  }), true);
  assert.equal(peerHasFreshPresence({
    activeLink: false,
    lastSeenAt: 0,
    nowMs: 31 * 60_000 + 1,
    announceIntervalSeconds: 1800,
    staleAfterMinutes: 30,
  }), false);
});

test("an active link is reachable without a recent announce", () => {
  assert.equal(peerHasFreshPresence({
    activeLink: true,
    lastSeenAt: undefined,
    nowMs: 24 * 60 * 60_000,
    announceIntervalSeconds: 1800,
    staleAfterMinutes: 30,
  }), true);
});

test("longer configured stale windows remain authoritative", () => {
  assert.equal(peerPresenceFreshnessMs(1800, 45), 45 * 60_000);
});
