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

export function buildStartupInterfaceItems(
  status: Pick<NodeStatus, "running" | "readiness">,
  _settings?: unknown,
): StartupInterfaceItem[] {
  const interfaces = Array.isArray(status.readiness?.interfaces)
    ? status.readiness.interfaces
    : [];
  return interfaces.map((record) => ({
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

export function statusHasRuntimeReceiveReadiness(
  status: Pick<NodeStatus, "running" | "readiness"> | null | undefined,
  _options?: { requiresInterfaceTelemetry?: boolean },
): boolean {
  return Boolean(status?.running) && status?.readiness?.state === "Ready";
}
