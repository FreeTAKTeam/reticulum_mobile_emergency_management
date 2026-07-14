export const LXMF_FIELD_COMMANDS = 0x09;
export const LXMF_FIELD_RESULTS = 0x0A;
export const LXMF_FIELD_EVENT = 0x0D;

export interface MissionCommandSource {
  rns_identity: string;
  display_name?: string;
}

export interface MissionCommandEnvelope {
  command_id: string;
  source: MissionCommandSource;
  timestamp: string;
  command_type: string;
  args: Record<string, unknown>;
  correlation_id?: string;
  topics: string[];
}

export interface MissionAcceptedPayload {
  command_id: string;
  status: "accepted";
  accepted_at: string;
  correlation_id?: string;
  by_identity?: string;
}

export interface MissionRejectedPayload {
  command_id: string;
  status: "rejected";
  reason_code: string;
  reason?: string;
  correlation_id?: string;
  required_capabilities?: string[];
}

export interface MissionResultPayload {
  command_id: string;
  status: "result";
  result: Record<string, unknown>;
  correlation_id?: string;
}

export type MissionResponsePayload =
  | MissionAcceptedPayload
  | MissionRejectedPayload
  | MissionResultPayload;

export interface MissionEventEnvelope {
  event_id: string;
  source: MissionCommandSource;
  timestamp: string;
  event_type: string;
  topics: string[];
  payload: Record<string, unknown>;
  meta?: Record<string, unknown>;
}

export interface ParsedMissionSyncFields {
  commands: MissionCommandEnvelope[];
  result: MissionResponsePayload | null;
  event: MissionEventEnvelope | null;
}
