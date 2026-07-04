import type { ActionMessage, EamStatus } from "../types/domain";

export type ActionMessageStatusField =
  | "securityStatus"
  | "capabilityStatus"
  | "preparednessStatus"
  | "medicalStatus"
  | "mobilityStatus"
  | "commsStatus";

export const ACTION_MESSAGE_STATUS_CONFIG: Array<{
  field: ActionMessageStatusField;
  label: string;
}> = [
  { field: "securityStatus", label: "Security" },
  { field: "capabilityStatus", label: "Capability" },
  { field: "preparednessStatus", label: "Preparedness" },
  { field: "medicalStatus", label: "Medical" },
  { field: "mobilityStatus", label: "Mobility" },
  { field: "commsStatus", label: "Comms" },
];

const STATUS_ROTATION: EamStatus[] = ["Unknown", "Green", "Yellow", "Red"];

function normalizeStatus(value: unknown): EamStatus {
  return value === "Green" || value === "Yellow" || value === "Red" ? value : "Unknown";
}

export function isActionMessageStatusField(value: keyof ActionMessage | string): value is ActionMessageStatusField {
  return ACTION_MESSAGE_STATUS_CONFIG.some((status) => status.field === value);
}

export function nextActionMessageStatus(value: unknown): EamStatus {
  const currentStatus = normalizeStatus(value);
  const currentIndex = STATUS_ROTATION.indexOf(currentStatus);
  return STATUS_ROTATION[(currentIndex + 1) % STATUS_ROTATION.length];
}

export function applyActionMessageStatusCycle(
  message: ActionMessage,
  field: keyof ActionMessage | string,
  updatedAt: number,
): ActionMessage | undefined {
  if (!isActionMessageStatusField(field)) {
    return undefined;
  }
  return {
    ...message,
    [field]: nextActionMessageStatus(message[field]),
    updatedAt,
  };
}
