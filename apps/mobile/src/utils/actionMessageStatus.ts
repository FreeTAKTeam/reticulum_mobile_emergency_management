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
