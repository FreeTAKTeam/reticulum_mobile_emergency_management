export type EamStatus = "Red" | "Yellow" | "Green" | "Unknown";

export interface ActionMessage {
  callsign: string;
  groupName: string;
  securityStatus: EamStatus;
  capabilityStatus: EamStatus;
  preparednessStatus: EamStatus;
  medicalStatus: EamStatus;
  mobilityStatus: EamStatus;
  commsStatus: EamStatus;
  notes?: string;
  updatedAt: number;
  deletedAt?: number;
}

export interface EventRecord {
  uid: string;
  callsign: string;
  type: string;
  summary: string;
  updatedAt: number;
  deletedAt?: number;
}

export type PeerSource = "announce" | "hub" | "import";
export type PeerConnectionState = "disconnected" | "connecting" | "connected";

export interface DiscoveredPeer {
  destination: string;
  label?: string;
  announcedName?: string;
  lastSeenAt: number;
  hops?: number;
  interfaceHex?: string;
  appData?: string;
  verifiedCapability: boolean;
  sources: PeerSource[];
  state: PeerConnectionState;
  lastError?: string;
}

export interface SavedPeer {
  destination: string;
  label?: string;
  savedAt: number;
}

export interface PeerListV1Peer {
  destination: string;
  label?: string;
}

export interface PeerListV1 {
  version: 1;
  generatedAt: string;
  capabilities: string[];
  peers: PeerListV1Peer[];
}

export interface HubSettings {
  mode: "Disabled" | "RchLxmf" | "RchHttp";
  identityHash: string;
  apiBaseUrl: string;
  apiKey: string;
  refreshIntervalSeconds: number;
}

export interface NodeUiSettings {
  displayName: string;
  clientMode: "auto" | "capacitor";
  autoConnectSaved: boolean;
  announceCapabilities: string;
  tcpClients: string[];
  broadcast: boolean;
  announceIntervalSeconds: number;
  showOnlyCapabilityVerified: boolean;
  hub: HubSettings;
}

export type ReplicationMessage =
  | {
      kind: "snapshot_request";
      requestedAt: number;
    }
  | {
      kind: "snapshot_response";
      requestedAt: number;
      messages: ActionMessage[];
    }
  | {
      kind: "message_upsert";
      message: ActionMessage;
    }
  | {
      kind: "message_delete";
      callsign: string;
      deletedAt: number;
    }
  | {
      kind: "event_snapshot_request";
      requestedAt: number;
    }
  | {
      kind: "event_snapshot_response";
      requestedAt: number;
      events: EventRecord[];
    }
  | {
      kind: "event_upsert";
      event: EventRecord;
    }
  | {
      kind: "event_delete";
      uid: string;
      deletedAt: number;
    };
