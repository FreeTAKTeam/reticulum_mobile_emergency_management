import {
  type NodeStatus,
  type PeerChangedEvent,
  type PeerRecord,
  type SendMode,
  type SyncStatus,
} from "@reticulum/node-client";

import type {
  HubRegistrationStatus,
  HubRegistryLinkage,
} from "../services/hubRegistryBootstrap";
import type {
  DiscoveredPeer,
  PeerConnectionState,
} from "../types/domain";
import { peerHasRemAnnounceEvidence } from "../utils/announceEvidence";

export const PEER_VISIBLE_UNSAVED_MAX_AGE_MS = 30 * 60_000;
export const PEER_PRESENCE_TICK_MS = 15_000;
export const EMPTY_BYTES = new Uint8Array(0);
export const STARTUP_ANNOUNCE_SETTLE_MS = 2_500;
export const NODE_START_TIMEOUT_MS = 15_000;
export const PROJECTION_REFRESH_DEBOUNCE_MS = 200;
export const OPERATIONAL_SUMMARY_REFRESH_MIN_INTERVAL_MS = 2_000;
export const REMOVED_PEERS_STORAGE_KEY = "reticulum.mobile.removedPeers.v1";
export const NODE_CONFIG_RESTART_REQUIRED_STORAGE_KEY = "reticulum.mobile.nodeConfigRestartRequired.v1";

export const EMPTY_STATUS: NodeStatus = {
  running: false,
  name: "",
  identityHex: "",
  appDestinationHex: "",
  lxmfDestinationHex: "",
  lastError: undefined,
  readiness: {
    state: "Pending",
    interfaces: [],
  },
  interfaces: [],
};

export const EMPTY_SYNC_STATUS: SyncStatus = {
  phase: "Idle",
  messagesReceived: 0,
};

export const EMPTY_OPERATIONAL_SUMMARY = {
  running: false,
  peerCountTotal: 0,
  savedPeerCount: 0,
  connectedPeerCount: 0,
  conversationCount: 0,
  messageCount: 0,
  eamCount: 0,
  eventCount: 0,
  telemetryCount: 0,
  updatedAtMs: 0,
};

export interface HubRegistrationSnapshot {
  status: HubRegistrationStatus;
  linkage?: HubRegistryLinkage;
  lastAttemptAt?: number;
  lastReadyAt?: number;
  lastError?: string;
}

export interface HubAnnounceCandidate {
  destination: string;
  label: string;
}
export interface UiLogLine {
  at: number;
  level: string;
  message: string;
}

export type EventPeerRoute = {
  appDestinationHex: string;
  lxmfDestinationHex: string;
  identityHex?: string;
  label?: string;
  announcedName?: string;
  sendMode: SendMode;
};
export type PacketSendOptions = {
  fieldsBase64?: string;
  sendMode?: SendMode;
};

export function shouldDisplayDiscoveredPeer(peer: DiscoveredPeer): boolean {
  if (peer.saved || peer.activeLink) {
    return true;
  }

  if (!hasActualRemAnnounce(peer) && !peer.sources.includes("hub")) {
    return false;
  }

  const seenAt = Math.max(peer.announceLastSeenAt ?? 0, peer.lxmfLastSeenAt ?? 0, peer.lastSeenAt ?? 0);
  return seenAt > 0 && (nowMs() - seenAt) <= PEER_VISIBLE_UNSAVED_MAX_AGE_MS;
}

export function hasActualRemAnnounce(peer: DiscoveredPeer): boolean {
  return peer.sources.includes("announce")
    && typeof peer.announceLastSeenAt === "number"
    && Number.isFinite(peer.announceLastSeenAt)
    && peer.announceLastSeenAt > 0
    && peerHasRemAnnounceEvidence(peer);
}

export function nowMs(): number {
  return Date.now();
}

export function advancePresenceNow(currentValue: number, candidateValue?: number): number {
  const candidate = typeof candidateValue === "number" && Number.isFinite(candidateValue)
    ? candidateValue
    : nowMs();
  return Math.max(currentValue, candidate);
}

export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

export function withTimeout<T>(operation: Promise<T>, timeoutMs: number, message: string): Promise<T> {
  let timerId: number | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timerId = window.setTimeout(() => {
      reject(new Error(message));
    }, timeoutMs);
  });

  return Promise.race([operation, timeout]).finally(() => {
    if (timerId !== undefined) {
      window.clearTimeout(timerId);
    }
  });
}

export function asTrimmedString(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

export function normalizeNodeStatus(value?: Partial<NodeStatus> | null): NodeStatus {
  const lastError = asTrimmedString(value?.lastError);
  return {
    running: Boolean(value?.running),
    name: typeof value?.name === "string" ? value.name : "",
    identityHex: typeof value?.identityHex === "string" ? value.identityHex : "",
    appDestinationHex: typeof value?.appDestinationHex === "string" ? value.appDestinationHex : "",
    lxmfDestinationHex: typeof value?.lxmfDestinationHex === "string" ? value.lxmfDestinationHex : "",
    lastError: lastError || undefined,
    readiness: value?.readiness ?? {
      state: "Pending",
      interfaces: [],
    },
    interfaces: Array.isArray(value?.interfaces) ? value.interfaces : [],
  };
}

export function activePropagationNodeHex(status: SyncStatus): string | undefined {
  const candidate = asTrimmedString(status.activePropagationNodeHex);
  return candidate ? candidate : undefined;
}

export function toUiPeerState(
  state: PeerRecord["state"] | PeerChangedEvent["change"]["state"] | undefined,
): PeerConnectionState {
  if (state === "Connected") {
    return "connected";
  }
  if (state === "Connecting") {
    return "connecting";
  }
  return "disconnected";
}

export function peerSortRank(peer: Pick<DiscoveredPeer, "saved" | "activeLink" | "lastSeenAt">): number {
  let rank = 0;
  if (peer.saved) {
    rank += 2;
  }
  if (peer.activeLink) {
    rank += 4;
  }
  if (peer.lastSeenAt > 0) {
    rank += 1;
  }
  return rank;
}
