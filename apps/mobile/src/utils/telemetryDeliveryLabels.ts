import type { TelemetryPublicationState } from "@reticulum/node-client";

const labels: Record<TelemetryPublicationState, string> = {
  StoredLocally: "Stored locally",
  QueuedNoRoute: "Waiting for route",
  QueuedForDelivery: "Queued for delivery",
  Transmitted: "Transmitted · Awaiting application acknowledgement",
  TransportDelivered: "Transport delivered · Awaiting application acknowledgement",
  Acknowledged: "Receiver accepted",
  Rejected: "Receiver rejected",
  TimedOut: "Acknowledgement timed out",
  Failed: "Delivery failed",
};

export function telemetryDeliveryLabel(state: TelemetryPublicationState): string { return labels[state]; }
