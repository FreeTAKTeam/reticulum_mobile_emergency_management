import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./readinessErrors.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const {
  hasConfiguredNonTcpInterface,
  logIndicatesReadinessError,
  nodeErrorIndicatesTcpInterfaceReadinessError,
  nodeErrorIndicatesReadinessError,
} = await import(moduleUrl);

test("intermediate direct LXMF send attempt errors do not mark the node not ready", () => {
  assert.equal(
    logIndicatesReadinessError(
      "[lxmf][mission] send attempt 1/5 errored destination=6521979f1165965b24731061ef4a6906 mode=Auto err=network error",
    ),
    false,
  );
});

test("intermediate propagation relay failures do not mark the node not ready", () => {
  assert.equal(
    logIndicatesReadinessError(
      "[lxmf][mission] propagation send relay attempt failed relay=fd74c182a6b5862e1360b8a61dcec8c4 destination=a133b8b1fe137f92210a048efded46db reason=network error",
    ),
    false,
  );
});

test("direct link activation failures do not mark the node not ready", () => {
  assert.equal(
    logIndicatesReadinessError(
      "[lxmf][events][sdk] link activation failed destination=fb4c70e20cfac047b899ca2f3671b50a attempt=3 reason=timeout",
    ),
    false,
  );
});

test("direct link activation network errors do not mark the node not ready", () => {
  assert.equal(
    nodeErrorIndicatesReadinessError({
      code: "NetworkError",
      message: "failed to activate lxmf link",
    }),
    false,
  );
});

test("final direct and propagation send failures do not mark the node not ready", () => {
  assert.equal(
    nodeErrorIndicatesReadinessError({
      code: "NetworkError",
      message: "LXMF send failed after direct and propagation attempts",
    }),
    false,
  );
});

test("send bytes delivery failures reported as invalid config do not mark the node not ready", () => {
  assert.equal(
    nodeErrorIndicatesReadinessError({
      code: "InvalidConfig",
      message: "send_bytes failed destination=5c231773f221c687682b031709c210fc reason=invalid config",
    }),
    false,
  );
});

test("plain invalid config action errors do not mark the node not ready", () => {
  assert.equal(
    nodeErrorIndicatesReadinessError({
      code: "InvalidConfig",
      message: "invalid send payload: missing bytes",
    }),
    false,
  );
});

test("delivery acknowledgement timeout does not mark the node not ready", () => {
  assert.equal(
    nodeErrorIndicatesReadinessError({
      code: "NetworkError",
      message: "lxmf delivery acknowledgement timeout destination=abc command=sos.status",
    }),
    false,
  );
});

test("unrecoverable node runtime failures still mark the node not ready", () => {
  assert.equal(
    nodeErrorIndicatesReadinessError({
      code: "InternalError",
      message: "node runtime failed unrecoverable bridge error",
    }),
    true,
  );
});

test("node runtime restore timeout marks the node not ready", () => {
  assert.equal(
    nodeErrorIndicatesReadinessError({
      code: "InternalError",
      message: "node runtime restore timed out after 15000ms",
    }),
    true,
  );
});

test("unreachable Reticulum TCP startup data path marks the node not ready", () => {
  assert.equal(
    nodeErrorIndicatesReadinessError({
      code: "NetworkError",
      message: "transport startup failed: no reachable Reticulum TCP interface endpoints=rns.beleth.net:4242",
    }),
    true,
  );
});

test("unreachable Reticulum TCP startup data path is classified as a TCP interface readiness error", () => {
  assert.equal(
    nodeErrorIndicatesTcpInterfaceReadinessError({
      code: "NetworkError",
      message: "transport startup failed: no reachable Reticulum TCP interface endpoints=rns.beleth.net:4242",
    }),
    true,
  );
});

test("enabled RNode with selected peripheral is a configured non-TCP interface", () => {
  assert.equal(
    hasConfiguredNonTcpInterface({
      rnode: {
        enabled: true,
        peripheralId: "48:CA:43:38:BC:E1",
      },
    }),
    true,
  );
});

test("disabled RNode is not a configured non-TCP interface", () => {
  assert.equal(
    hasConfiguredNonTcpInterface({
      rnode: {
        enabled: false,
        peripheralId: "48:CA:43:38:BC:E1",
      },
    }),
    false,
  );
});
