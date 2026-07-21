import type { NodeStatus } from "@reticulum/node-client";

export type StartupInterfaceState = "disabled" | "loading" | "ready" | "failed" | "unsupported";

export const STARTUP_INTERFACE_LOADING_SUMMARY = "Interfaces are loading";
export const STARTUP_INTERFACE_LOADING_DETAIL = "Waiting for configured interfaces to finish starting.";

export interface StartupInterfaceItem {
  id: string;
  label: string;
  detail: string;
  state: StartupInterfaceState;
}

interface RuntimeReadinessRecordLike {
  id: string;
  label: string;
  detail: string;
  lastError?: string;
  state: string;
}

function isRuntimeReadinessRecord(value: unknown): value is RuntimeReadinessRecordLike {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return false;
  }
  const record = value as Partial<RuntimeReadinessRecordLike>;
  return typeof record.id === "string"
    && record.id.trim().length > 0
    && typeof record.label === "string"
    && record.label.trim().length > 0
    && typeof record.detail === "string"
    && typeof record.state === "string"
    && (record.lastError === undefined || typeof record.lastError === "string");
}

export function buildStartupInterfaceItems(
  status: Pick<NodeStatus, "running" | "readiness">,
  _settings?: unknown,
): StartupInterfaceItem[] {
  const interfaces = Array.isArray(status.readiness?.interfaces)
    ? status.readiness.interfaces
    : [];
  return interfaces.filter(isRuntimeReadinessRecord).map((record) => ({
    id: record.id,
    label: record.label,
    detail: record.lastError || record.detail,
    state: runtimeStateToStartupState(record.state),
  }));
}

function runtimeStateToStartupState(state: string): StartupInterfaceState {
  switch (state) {
    case "Ready":
      return "ready";
    case "Failed":
      return "failed";
    case "Unsupported":
      return "unsupported";
    case "Disabled":
      return "disabled";
    case "Pending":
    default:
      return "loading";
  }
}

export function statusHasRuntimeStartupReadiness(
  status: Pick<NodeStatus, "running" | "readiness"> | null | undefined,
  _options?: { requiresInterfaceTelemetry?: boolean },
): boolean {
  return Boolean(status?.running) && status?.readiness?.state === "Ready";
}

export function statusNeedsStartupInterfaceGrace(
  status: Pick<NodeStatus, "running" | "readiness"> | null | undefined,
  items: readonly StartupInterfaceItem[],
): boolean {
  if (!status?.running || statusHasRuntimeStartupReadiness(status)) {
    return false;
  }
  return items.some((item) => item.id !== "local" && item.state !== "disabled");
}
