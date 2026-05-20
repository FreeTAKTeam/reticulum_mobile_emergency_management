import type { NodeErrorEvent } from "@reticulum/node-client";

const READINESS_ERROR_LOG_PATTERNS = [
  /\bNetworkError\b/i,
  /\bsend_(?:bytes|lxmf) failed\b/i,
  /\bretry_lxmf failed\b/i,
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
  if (logIndicatesPropagationRelayError(message)) {
    return false;
  }
  if (/\blink activation (?:failed|retry)\b/i.test(message)) {
    return false;
  }
  if (/\bsend attempt\b.*\b(?:failed|errored)\b/i.test(message)) {
    return false;
  }
  return READINESS_ERROR_LOG_PATTERNS.some((pattern) => pattern.test(message));
}

export function nodeErrorIndicatesReadinessError(event: NodeErrorEvent): boolean {
  const message = `${event.code}: ${event.message}`;
  if (logIndicatesPropagationRelayError(message)) {
    return false;
  }
  if (/\bfailed to activate lxmf link\b/i.test(message)) {
    return false;
  }
  return event.code === "NetworkError" || logIndicatesReadinessError(message);
}
