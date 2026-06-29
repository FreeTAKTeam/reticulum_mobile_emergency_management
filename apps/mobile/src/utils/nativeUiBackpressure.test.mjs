import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./nativeUiBackpressure.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const {
  nativeLogShouldAppendToUi,
} = await import(moduleUrl);

test("packet-level transport diagnostics do not append to reactive UI logs", () => {
  assert.equal(
    nativeLogShouldAppendToUi(
      "Info",
      "[tp-diag] inbound_packet node=S8 iface=/abc/ type=Data hash=123",
    ),
    false,
  );
  assert.equal(
    nativeLogShouldAppendToUi(
      "Info",
      "[iface][rx] endpoint=<rmap.world:4242> packets=224 bytes=14473 announces=28",
    ),
    false,
  );
});

test("high-rate announce and link maintenance logs do not append to reactive UI logs", () => {
  assert.equal(
    nativeLogShouldAppendToUi(
      "Info",
      "[announceReceived] {\"destinationHex\":\"abc\",\"destinationKind\":\"other\"}",
    ),
    false,
  );
  assert.equal(
    nativeLogShouldAppendToUi(
      "Info",
      "[link][maintain] destination=4411 status=connecting kind=LxmfDelivery",
    ),
    false,
  );
  assert.equal(
    nativeLogShouldAppendToUi(
      "Info",
      "[lxmf][events] link activation retry destination=a1c8126d7cb806e6bde086d582b6cb0d attempt=2 timeout_ms=20000 reason=timeout",
    ),
    false,
  );
});

test("actionable native warnings and errors still append to UI logs", () => {
  assert.equal(
    nativeLogShouldAppendToUi("Warn", "transport startup failed: no reachable Reticulum TCP interface"),
    true,
  );
  assert.equal(
    nativeLogShouldAppendToUi("Error", "node runtime failed unrecoverable bridge error"),
    true,
  );
});
