# REM Developer Examples

These examples target `@reticulum/node-client` as shipped with REM
`1.2.7-rc.1`. Existing asynchronous client methods are unchanged; native
rejections are now consistently classifiable.

## Classify A Node Error

```ts
import {
  classifyNodeError,
  createReticulumNodeClient,
} from "@reticulum/node-client";

const client = createReticulumNodeClient({ mode: "capacitor" });

try {
  await client.announceNow();
} catch (unknownError: unknown) {
  const error = classifyNodeError(unknownError, "announceNow");
  console.error(error.code, error.message, error.operation, error.cause);
}
```

`code` is stable and `operation` identifies the failed bridge call when it is
known. `cause` is diagnostic data and must not be used as a stable programmatic
key.

## Retry Only Retryable Failures

```ts
import { classifyNodeError } from "@reticulum/node-client";

async function retryOnce<T>(operation: string, action: () => Promise<T>): Promise<T> {
  try {
    return await action();
  } catch (unknownError: unknown) {
    const error = classifyNodeError(unknownError, operation);
    if (!error.retryable) throw error;
    await new Promise((resolve) => setTimeout(resolve, 1_000));
    return action();
  }
}
```

Retryable categories include transient I/O, network, Reticulum, timeout, and
event-stream failures. Invalid configuration, packet construction, and internal
invariant failures require correction or operator attention instead of a loop.
Every retry policy must be bounded and cancellation-aware.

## Inspect Runtime Readiness

```ts
const status = await client.getStatus();

if (status.readiness.state === "Ready") {
  // The local Rust runtime is usable. Individual interfaces can still be degraded.
}

for (const entry of status.readiness.interfaces) {
  if (entry.state === "Failed" || entry.state === "Unsupported") {
    console.warn(entry.id, entry.state, entry.detail);
  }
}
```

Do not hold the application splash open for a failed RCH refresh or network
interface after the aggregate local runtime becomes `Ready`.

## Handle Delivery Terminal States

```ts
import type { MessageState } from "@reticulum/node-client";

const terminalStates = new Set<MessageState>([
  "Delivered",
  "Failed",
  "TimedOut",
  "Cancelled",
  "Received",
]);

function isTerminal(state: MessageState): boolean {
  return terminalStates.has(state);
}
```

`Queued`, `PathRequested`, `LinkEstablishing`, `Sending`, `SentDirect`, and
`SentToPropagation` are intermediate. A transport receipt and an application
acknowledgement are different signals; do not render an operational command as
completed until its required application acknowledgement reaches a terminal
accepted/completed state.
