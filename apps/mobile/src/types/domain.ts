export type EamStatus = "Red" | "Yellow" | "Green" | "Unknown";
export type EamWireStatus = Exclude<EamStatus, "Unknown">;

export const EAM_COMMAND_TYPES = [
  "mission.registry.eam.list",
  "mission.registry.eam.upsert",
  "mission.registry.eam.get",
  "mission.registry.eam.latest",
  "mission.registry.eam.delete",
  "mission.registry.eam.team.summary",
] as const;

export const EAM_EVENT_TYPES = [
  "mission.registry.eam.listed",
  "mission.registry.eam.upserted",
  "mission.registry.eam.retrieved",
  "mission.registry.eam.latest_retrieved",
  "mission.registry.eam.deleted",
  "mission.registry.eam.team_summary.retrieved",
] as const;

export type EamCommandType = (typeof EAM_COMMAND_TYPES)[number];
export type EamEventType = (typeof EAM_EVENT_TYPES)[number];

export interface EamSource {
  rns_identity: string;
  display_name?: string;
}

export interface EamStatusFields {
  security_status?: EamWireStatus;
  capability_status?: EamWireStatus;
  preparedness_status?: EamWireStatus;
  medical_status?: EamWireStatus;
  mobility_status?: EamWireStatus;
  comms_status?: EamWireStatus;
}

export interface EamRecord extends EamStatusFields {
  eam_uid?: string;
  callsign: string;
  team_member_uid: string;
  team_uid: string;
  reported_by?: string;
  reported_at?: string;
  overall_status?: EamWireStatus;
  notes?: string;
  confidence?: number;
  ttl_seconds?: number;
  source?: EamSource;
}

export interface EamCommandContext {
  eam_uid?: string;
  callsign?: string;
  team_uid?: string;
  team_member_uid?: string;
}

export interface EamListCommandArgs extends EamCommandContext {
  limit?: number;
  offset?: number;
  include_deleted?: boolean;
}

export interface EamUpsertCommandArgs extends EamStatusFields {
  callsign: string;
  team_member_uid: string;
  team_uid: string;
  eam_uid?: string;
  reported_by?: string;
  reported_at?: string;
  notes?: string;
  confidence?: number;
  ttl_seconds?: number;
  source?: EamSource;
}

export interface EamGetCommandArgs extends EamCommandContext {}

export interface EamLatestCommandArgs extends EamCommandContext {}

export interface EamDeleteCommandArgs extends EamCommandContext {}

export interface EamTeamSummaryCommandArgs {
  team_uid: string;
  team_member_uid?: string;
  callsign?: string;
}

export interface EamListResult {
  eams: EamRecord[];
}

export interface EamEntityResult {
  eam: EamRecord | null;
}

export interface EamDeleteResult {
  eam: EamRecord | null;
  status?: "deleted" | "not_found";
  eam_uid?: string;
  callsign?: string;
}

export interface EamTeamSummary {
  team_uid: string;
  team_name?: string;
  total?: number;
  active_total?: number;
  deleted_total?: number;
  overall_status?: EamWireStatus;
  by_status?: Partial<Record<EamWireStatus, number>>;
  updated_at?: string;
  [key: string]: unknown;
}

export interface EamTeamSummaryResult {
  summary: EamTeamSummary;
}

export interface EamCommandArgsByType {
  "mission.registry.eam.list": EamListCommandArgs;
  "mission.registry.eam.upsert": EamUpsertCommandArgs;
  "mission.registry.eam.get": EamGetCommandArgs;
  "mission.registry.eam.latest": EamLatestCommandArgs;
  "mission.registry.eam.delete": EamDeleteCommandArgs;
  "mission.registry.eam.team.summary": EamTeamSummaryCommandArgs;
}

export interface EamResultByCommandType {
  "mission.registry.eam.list": EamListResult;
  "mission.registry.eam.upsert": EamEntityResult;
  "mission.registry.eam.get": EamEntityResult;
  "mission.registry.eam.latest": EamEntityResult;
  "mission.registry.eam.delete": EamDeleteResult;
  "mission.registry.eam.team.summary": EamTeamSummaryResult;
}

export interface EamEventPayloadByType {
  "mission.registry.eam.listed": EamListResult;
  "mission.registry.eam.upserted": EamEntityResult;
  "mission.registry.eam.retrieved": EamEntityResult;
  "mission.registry.eam.latest_retrieved": EamEntityResult;
  "mission.registry.eam.deleted": EamDeleteResult;
  "mission.registry.eam.team_summary.retrieved": EamTeamSummaryResult;
}

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
  eamUid?: string;
  teamMemberUid?: string;
  teamUid?: string;
  reportedAt?: string;
  reportedBy?: string;
  overallStatus?: EamWireStatus;
  confidence?: number;
  ttlSeconds?: number;
  source?: EamSource;
  syncState?: "draft" | "syncing" | "synced" | "error";
  syncError?: string;
  draftCreatedAt?: number;
  lastSyncedAt?: number;
}

export interface EventSource {
  rns_identity: string;
  display_name?: string;
}

export interface EventArgs {
  entry_uid: string;
  mission_uid: string;
  content: string;
  callsign: string;
  server_time?: string;
  client_time?: string;
  keywords: string[];
  content_hashes: string[];
  source_identity?: string;
  source_display_name?: string;
}

export interface EventRecord {
  command_id: string;
  source: EventSource;
  timestamp: string;
  command_type: string;
  args: EventArgs;
  correlation_id?: string;
  topics: string[];
  deleted_at?: number;
}

export interface TelemetryPosition {
  callsign: string;
  lat: number;
  lon: number;
  alt?: number;
  course?: number;
  speed?: number;
  accuracy?: number;
  updatedAt: number;
}

export type PeerSource = "announce" | "hub" | "import";
export type PeerConnectionState = "disconnected" | "connecting" | "connected";

export interface DiscoveredPeer {
  destination: string;
  identityHex?: string;
  lxmfDestinationHex?: string;
  announceLastSeenAt?: number;
  lxmfLastSeenAt?: number;
  label?: string;
  announcedName?: string;
  lastSeenAt: number;
  hops?: number;
  interfaceHex?: string;
  appData?: string;
  sources: PeerSource[];
  state: PeerConnectionState;
  saved: boolean;
  stale: boolean;
  activeLink: boolean;
  lastError?: string;
  lastResolutionError?: string;
  lastResolutionAttemptAt?: number;
}

export interface SavedPeer {
  destination: string;
  label?: string;
  savedAt: number;
}

export type HubMode = "Autonomous" | "SemiAutonomous" | "Connected";

export interface HubDirectoryPeerRecord {
  identity: string;
  destinationHash: string;
  displayName?: string;
  announceCapabilities: string[];
  clientType?: string;
  registeredMode?: string;
  lastSeen?: string;
  status?: string;
}

export interface HubDirectorySnapshot {
  effectiveConnectedMode: boolean;
  items: HubDirectoryPeerRecord[];
  receivedAtMs: number;
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
  mode: HubMode;
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
  telemetry: {
    enabled: boolean;
    publishIntervalSeconds: number;
    accuracyThresholdMeters?: number;
    staleAfterMinutes: number;
    expireAfterMinutes: number;
  };
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
    }
  | {
      kind: "telemetry_upsert";
      position: TelemetryPosition;
    }
  | {
      kind: "telemetry_delete";
      callsign: string;
      deletedAt: number;
    }
  | {
      kind: "telemetry_snapshot_request";
      requestedAt: number;
    }
  | {
      kind: "telemetry_snapshot_response";
      requestedAt: number;
      positions: TelemetryPosition[];
    };
