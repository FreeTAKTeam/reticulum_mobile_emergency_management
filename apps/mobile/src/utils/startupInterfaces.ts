import type { InterfaceStatusRecord, NodeStatus } from "@reticulum/node-client";

export type StartupInterfaceState = "disabled" | "loading" | "waiting" | "ready";

export const STARTUP_INTERFACE_LOADING_SUMMARY = "Interfaces are loading";
export const STARTUP_INTERFACE_LOADING_DETAIL = "Waiting for active links to report traffic.";

export interface StartupInterfaceItem {
  id: string;
  label: string;
  detail: string;
  state: StartupInterfaceState;
}

interface StartupInterfaceSettings {
  tcpClients?: unknown;
  rnode?: {
    enabled?: unknown;
    connectionMode?: unknown;
    peripheralId?: unknown;
  } | null;
}

function normalizedInterfaceKind(value: string): string {
  return value.trim().toLowerCase();
}

function interfaceIsRnode(record: Pick<InterfaceStatusRecord, "kind" | "label">): boolean {
  const kind = normalizedInterfaceKind(record.kind);
  const label = record.label.trim().toLowerCase();
  return kind === "rnode_ble" || kind === "rnode" || label.startsWith("rnode-ble:");
}

function interfaceIsTcp(record: Pick<InterfaceStatusRecord, "kind" | "label">): boolean {
  const kind = normalizedInterfaceKind(record.kind);
  return kind === "tcp_client" || kind === "tcp";
}

function interfaceHasTraffic(
  record: Pick<InterfaceStatusRecord, "rxPackets" | "rxBytes" | "lastActivityMs">,
): boolean {
  return Number(record.rxPackets) > 0
    || Number(record.rxBytes) > 0
    || Number(record.lastActivityMs) > 0;
}

function interfaceIsConnected(record: Pick<InterfaceStatusRecord, "state">): boolean {
  return record.state.trim().toLowerCase() === "connected";
}

function interfaceIsReceiving(
  record: Pick<InterfaceStatusRecord, "state" | "rxPackets" | "rxBytes" | "lastActivityMs">,
): boolean {
  return interfaceIsConnected(record) && interfaceHasTraffic(record);
}

function tcpConfigured(settings: StartupInterfaceSettings): boolean {
  return Array.isArray(settings.tcpClients)
    && settings.tcpClients.some((entry) => String(entry ?? "").trim().length > 0);
}

function loraConfigured(settings: StartupInterfaceSettings): boolean {
  const rnode = settings.rnode;
  const connectionMode = String(rnode?.connectionMode ?? "ble").trim().toLowerCase();
  if (connectionMode === "tcp" || connectionMode === "wifi" || connectionMode === "wi-fi") {
    return false;
  }
  return Boolean(rnode?.enabled) && String(rnode?.peripheralId ?? "").trim().length > 0;
}

function configuredInterfaceState(
  configured: boolean,
  running: boolean,
  records: InterfaceStatusRecord[],
): StartupInterfaceState {
  if (!configured) {
    return "disabled";
  }
  if (!running || records.length === 0) {
    return "loading";
  }
  if (records.some(interfaceIsReceiving)) {
    return "ready";
  }
  if (records.some(interfaceIsConnected)) {
    return "waiting";
  }
  return "loading";
}

function detailForState(
  state: StartupInterfaceState,
  details: Record<StartupInterfaceState, string>,
): string {
  return details[state];
}

export function buildStartupInterfaceItems(
  status: Pick<NodeStatus, "running" | "interfaces">,
  settings: StartupInterfaceSettings,
): StartupInterfaceItem[] {
  const interfaces = Array.isArray(status.interfaces) ? status.interfaces : [];
  const running = Boolean(status.running);
  const loraRecords = interfaces.filter(interfaceIsRnode);
  const tcpRecords = interfaces.filter(interfaceIsTcp);
  const trafficReady = statusHasReceivingInterface(status);
  const anyInterfaceReported = interfaces.length > 0;
  const loraState = configuredInterfaceState(loraConfigured(settings), running, loraRecords);
  const tcpState = configuredInterfaceState(tcpConfigured(settings), running, tcpRecords);
  const reticulumState: StartupInterfaceState = !running
    ? "loading"
    : trafficReady
      ? "ready"
      : anyInterfaceReported
        ? "waiting"
        : "loading";

  return [
    {
      id: "rnode",
      label: "LoRa",
      detail: detailForState(loraState, {
        disabled: "No LoRa interface activated",
        loading: "Starting radio interface",
        waiting: "Connected, waiting for received packets",
        ready: "Receiving LoRa traffic",
      }),
      state: loraState,
    },
    {
      id: "tcp",
      label: "TCP community",
      detail: detailForState(tcpState, {
        disabled: "No TCP server activated",
        loading: "Starting TCP interface",
        waiting: "Connected, waiting for received packets",
        ready: "Receiving TCP traffic",
      }),
      state: tcpState,
    },
    {
      id: "local",
      label: "Reticulum Net",
      detail: detailForState(reticulumState, {
        disabled: "Runtime disabled",
        loading: "Starting runtime",
        waiting: "Waiting for interface traffic",
        ready: "At least one interface is receiving",
      }),
      state: reticulumState,
    },
  ];
}

export function statusHasReceivingInterface(
  status: Pick<NodeStatus, "interfaces"> | null | undefined,
): boolean {
  const interfaces = Array.isArray(status?.interfaces) ? status.interfaces : [];
  return interfaces.some(interfaceIsReceiving);
}
