export type TelemetryPublicationState =
  | "StoredLocally" | "QueuedNoRoute" | "QueuedForDelivery" | "Transmitted"
  | "TransportDelivered" | "Acknowledged" | "Rejected" | "TimedOut" | "Failed";

export interface TelemetryRecipientPublication {
  destinationHex: string;
  callsign: string;
  fixUpdatedAt: number;
  commandId: string;
  state: TelemetryPublicationState;
  messageIdHex: string | null;
  representation: string | null;
  detail: string | null;
  updatedAt: number;
}

export interface TelemetryReceiveFailure {
  sourceHex: string | null;
  destinationHex: string;
  messageIdHex: string | null;
  commandId: string | null;
  reason: string;
  receivedAt: number;
}

export interface TelemetryPublicationSnapshot {
  localFixUpdatedAt: number | null;
  callsign: string | null;
  recipients: TelemetryRecipientPublication[];
  lastReceiveFailure: TelemetryReceiveFailure | null;
}
