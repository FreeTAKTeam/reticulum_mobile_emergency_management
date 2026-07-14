import type {
  ApplicationAckState,
  MessageDirection,
  MessageMethod,
  MessageState,
  TransportDeliveryState,
} from "@reticulum/node-client";

export const MESSAGE_METHODS = new Set<MessageMethod>([
  "Direct",
  "Opportunistic",
  "Propagated",
  "Resource",
]);
export const MESSAGE_STATES = new Set<MessageState>([
  "Queued",
  "PathRequested",
  "LinkEstablishing",
  "Sending",
  "SentDirect",
  "SentToPropagation",
  "Delivered",
  "Failed",
  "TimedOut",
  "Cancelled",
  "Received",
]);
export const MESSAGE_DIRECTIONS = new Set<MessageDirection>(["Inbound", "Outbound"]);
export const TRANSPORT_DELIVERY_STATES = new Set<TransportDeliveryState>([
  "Queued",
  "Sending",
  "SentDirect",
  "SentToPropagation",
  "TransportDelivered",
  "Failed",
  "TimedOut",
  "Cancelled",
]);
export const APPLICATION_ACK_STATES = new Set<ApplicationAckState>([
  "NotRequired",
  "Waiting",
  "Accepted",
  "Completed",
  "Rejected",
  "Failed",
]);
