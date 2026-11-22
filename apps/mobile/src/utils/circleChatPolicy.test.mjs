import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

async function loadModule(name, replacements = {}) {
  let source = await readFile(new URL(`./${name}.ts`, import.meta.url), "utf8");
  for (const [from, to] of Object.entries(replacements)) {
    source = source.replace(from, to);
  }
  const output = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  return `data:text/javascript;base64,${Buffer.from(output).toString("base64")}`;
}

const stringValuesUrl = await loadModule("stringValues");
const inboxPeerOptionsUrl = await loadModule("inboxPeerOptions", { "./stringValues": stringValuesUrl });
const { chatPolicyReason } = await import(await loadModule("circleChatPolicy", {
  "./inboxPeerOptions": inboxPeerOptionsUrl,
  "./stringValues": stringValuesUrl,
}));

const hub = "a".repeat(32);
const stranger = "b".repeat(32);

test("chat with an unsaved peer is blocked", () => {
  assert.match(chatPolicyReason(false, stranger, undefined, hub), /Inner Circle/);
});

test("chat with the configured hub is allowed without a saved peer", () => {
  assert.equal(chatPolicyReason(false, hub, undefined, hub.toUpperCase()), "");
});

test("power saver still pauses hub chat", () => {
  assert.match(chatPolicyReason(true, hub, undefined, hub), /Power saver/);
});
