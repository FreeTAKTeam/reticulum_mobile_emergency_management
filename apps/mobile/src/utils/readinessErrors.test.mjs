import assert from "node:assert/strict";
import { test } from "node:test";

import {
  logIndicatesReadinessError,
  nodeErrorIndicatesReadinessError,
} from "../../../../tmp/readiness-errors/readinessErrors.js";

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

test("final network errors still mark the node not ready", () => {
  assert.equal(
    nodeErrorIndicatesReadinessError({
      code: "NetworkError",
      message: "LXMF send failed after direct and propagation attempts",
    }),
    true,
  );
});

test("delivery acknowledgement timeout marks the node not ready", () => {
  assert.equal(
    nodeErrorIndicatesReadinessError({
      code: "NetworkError",
      message: "lxmf delivery acknowledgement timeout destination=abc command=sos.status",
    }),
    true,
  );
});
