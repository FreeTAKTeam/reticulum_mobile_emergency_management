import type { NodeErrorEvent } from "@reticulum/node-client";

const GLOBAL_READINESS_ERROR_LOG_PATTERNS = [
  /\bnode runtime failed\b/i,
  /\bunrecoverable\b/i,
  /\bsdk_start_failed\b/i,
  /\b(?:native )?bridge\b.*\b(?:failed|error|unavailable)\b/i,
  /\b(?:storage|database|app state)\b.*\b(?:failed|error|corrupt|unavailable)\b/i,
  /\b(?:transport|node)\b.*\b(?:startup|start)\b.*\bfailed\b/i,
];

const DELIVERY_ERROR_LOG_PATTERNS = [
  /\bLXMF send failed after\b/i,
  /\ball available direct\/propagation attempts\b/i,
  /\bdirect and propagation attempts\b/i,
  /\blxmf delivery acknowledgement timeout\b/i,
  /\bfailed to activate lxmf link\b/i,
  /\blink activation (?:failed|retry)\b/i,
  /\bsend attempt\b.*\b(?:failed|errored)\b/i,
  /\bpropagation send relay attempt failed\b/i,
  /\bpropagation relay\b.*\b(?:failed|error|errored)\b/i,
  /\bretry_lxmf failed\b/i,
  /\bsend_lxmf failed\b/i,
  /\bsend_bytes failed\b/i,
  /\bbroadcast_bytes failed\b/i,
];

const PROPAGATION_RELAY_ERROR_LOG_PATTERNS = [
  /\bpropagation send relay attempt failed\b/i,
  /\bpropagation relay\b.*\b(?:failed|error|errored)\b/i,
];

export function logIndicatesPropagationRelayError(message: string): boolean {
  return PROPAGATION_RELAY_ERROR_LOG_PATTERNS.some((pattern) => pattern.test(message));
}

export function logIndicatesReadinessError(message: string): boolean {
  if (DELIVERY_ERROR_LOG_PATTERNS.some((pattern) => pattern.test(message))) {
    return false;
  }
  return GLOBAL_READINESS_ERROR_LOG_PATTERNS.some((pattern) => pattern.test(message));
}

export function nodeErrorIndicatesReadinessError(event: NodeErrorEvent): boolean {
  const message = `${event.code}: ${event.message}`;
  if (DELIVERY_ERROR_LOG_PATTERNS.some((pattern) => pattern.test(message))) {
    return false;
  }
  if (logIndicatesReadinessError(message)) {
    return true;
  }
  return event.code === "InternalError"
    || event.code === "IoError"
    || event.code === "InvalidConfig"
    || event.code === "NotRunning";
}
