import type { InterfaceStatusRecord, NodeErrorEvent, NodeStatus } from "@reticulum/node-client";

export type RnodeInterfaceSeverity = "disabled" | "pending" | "blocking" | "degraded" | "ready";

export interface RnodeInterfaceSummary {
  severity: RnodeInterfaceSeverity;
  message?: string;
  notificationLabel?: string;
  rnodeConfigured: boolean;
  rnodeAvailable: boolean;
  otherAvailableCount: number;
}

const GLOBAL_READINESS_ERROR_LOG_PATTERNS = [
  /\bnode runtime failed\b/i,
  /\bunrecoverable\b/i,
  /\bsdk_start_failed\b/i,
  /\bnode runtime\b.*\b(?:timed out|timeout|failed|crash|anr)\b/i,
  /\bruntime restore\b.*\b(?:timed out|timeout|failed)\b/i,
  /\b(?:native )?bridge\b.*\b(?:failed|error|unavailable)\b/i,
  /\b(?:storage|database|app state)\b.*\b(?:failed|error|corrupt|unavailable)\b/i,
  /\b(?:transport|node)\b.*\b(?:startup|start)\b.*\bfailed\b/i,
];

const TCP_INTERFACE_READINESS_ERROR_LOG_PATTERNS = [
  /\bno reachable Reticulum TCP interface\b/i,
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
  /\bsend_bytes failed\b.*\breason=invalid config\b/i,
  /\bbroadcast_bytes failed\b/i,
  /\b(?:event|event delete|checklist|eam|eam delete|telemetry|sos) replication (?:enqueue|delivery) failed\b/i,
];

const PER_DESTINATION_REPLICATION_FAILURE_PATTERN =
  /\b(?:event|event delete|checklist|eam|eam delete|telemetry|sos) replication (?:enqueue|delivery) failed\b.*\bdestination=[0-9a-f]{32}\b/i;

const PROPAGATION_RELAY_ERROR_LOG_PATTERNS = [
  /\bpropagation send relay attempt failed\b/i,
  /\bpropagation relay\b.*\b(?:failed|error|errored)\b/i,
];

export function logIndicatesPropagationRelayError(message: string): boolean {
  return PROPAGATION_RELAY_ERROR_LOG_PATTERNS.some((pattern) => pattern.test(message));
}

export function hasConfiguredNonTcpInterface(
  settings: { rnode?: { enabled?: unknown; connectionMode?: unknown; peripheralId?: unknown } | null } | null | undefined,
): boolean {
  const rnode = settings?.rnode;
  const connectionMode = String(rnode?.connectionMode ?? "ble").trim().toLowerCase();
  if (connectionMode === "tcp" || connectionMode === "wifi" || connectionMode === "wi-fi") {
    return false;
  }
  return rnode?.enabled === true && String(rnode?.peripheralId ?? "").trim().length > 0;
}

function interfaceIsRnodeBluetooth(
  record: Partial<Pick<InterfaceStatusRecord, "kind" | "label">> | null | undefined,
): boolean {
  const kind = typeof record?.kind === "string" ? record.kind.trim().toLowerCase() : "";
  const label = typeof record?.label === "string" ? record.label.trim().toLowerCase() : "";
  return kind === "rnode_ble"
    || kind === "rnode_bluetooth_classic"
    || label.startsWith("rnode-ble:")
    || label.startsWith("rnode-bluetooth-classic:");
}

function interfaceIsAvailable(
  record: Partial<Pick<InterfaceStatusRecord, "state">> | null | undefined,
): boolean {
  return typeof record?.state === "string"
    && record.state.trim().toLowerCase() === "connected";
}

export function summarizeRnodeInterfaceState(
  status: Pick<NodeStatus, "running" | "interfaces"> | null | undefined,
  settings: { rnode?: { enabled?: unknown; connectionMode?: unknown; peripheralId?: unknown } | null } | null | undefined,
): RnodeInterfaceSummary {
  const interfaces = Array.isArray(status?.interfaces) ? status.interfaces : [];
  const rnodeAvailable = interfaces.some((entry) => interfaceIsRnodeBluetooth(entry) && interfaceIsAvailable(entry));
  const otherAvailableCount = interfaces.filter((entry) => !interfaceIsRnodeBluetooth(entry) && interfaceIsAvailable(entry)).length;
  const anyInterfaceAvailable = rnodeAvailable || otherAvailableCount > 0;
  const rnodeConfigured = hasConfiguredNonTcpInterface(settings);

  if (status?.running && anyInterfaceAvailable) {
    return {
      severity: "ready",
      rnodeConfigured,
      rnodeAvailable,
      otherAvailableCount,
    };
  }

  if (!rnodeConfigured) {
    return {
      severity: "disabled",
      rnodeConfigured: false,
      rnodeAvailable: false,
      otherAvailableCount: 0,
    };
  }
  if (!status?.running) {
    return {
      severity: "pending",
      rnodeConfigured: true,
      rnodeAvailable: false,
      otherAvailableCount: 0,
    };
  }
  if (!rnodeAvailable) {
    return {
      severity: "degraded",
      message: "RNode LoRa is configured but unavailable. REM is running and will keep retrying the interface.",
      notificationLabel: "RNode unavailable; REM is still running",
      rnodeConfigured: true,
      rnodeAvailable,
      otherAvailableCount,
    };
  }
  return {
    severity: "ready",
    rnodeConfigured: true,
    rnodeAvailable,
    otherAvailableCount,
  };
}

export function logIndicatesTcpInterfaceReadinessError(message: string): boolean {
  return TCP_INTERFACE_READINESS_ERROR_LOG_PATTERNS.some((pattern) => pattern.test(message));
}

export function logIndicatesReadinessError(message: string): boolean {
  if (DELIVERY_ERROR_LOG_PATTERNS.some((pattern) => pattern.test(message))) {
    return false;
  }
  if (TCP_INTERFACE_READINESS_ERROR_LOG_PATTERNS.some((pattern) => pattern.test(message))) {
    return false;
  }
  return GLOBAL_READINESS_ERROR_LOG_PATTERNS.some((pattern) => pattern.test(message));
}

export function nodeErrorIndicatesTcpInterfaceReadinessError(event: NodeErrorEvent): boolean {
  return logIndicatesTcpInterfaceReadinessError(`${event.code}: ${event.message}`);
}

export function nodeErrorIndicatesPerDestinationDeliveryFailure(event: NodeErrorEvent): boolean {
  return PER_DESTINATION_REPLICATION_FAILURE_PATTERN.test(event.message);
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
    || event.code === "NotRunning";
}
