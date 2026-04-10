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

const STATUS_SCORES: Record<EamStatus, number> = {
  Green: 100,
  Yellow: 50,
  Red: 25,
  Unknown: 0,
};

function clampScore(value: number): number {
  return Math.max(0, Math.min(100, Math.round(value)));
}

function blendHexColor(start: string, end: string, ratio: number): string {
  const safeRatio = Math.max(0, Math.min(1, ratio));
  const startChannels = start.match(/[a-f0-9]{2}/gi);
  const endChannels = end.match(/[a-f0-9]{2}/gi);

  if (!startChannels || !endChannels) {
    return end;
  }

  const mixedChannels = startChannels.map((channel, index) => {
    const startValue = Number.parseInt(channel, 16);
    const endValue = Number.parseInt(endChannels[index], 16);
    const nextValue = Math.round(startValue + ((endValue - startValue) * safeRatio));
    return nextValue.toString(16).padStart(2, "0");
  });

  return `#${mixedChannels.join("")}`;
}

export function getStatusScore(status: EamStatus): number {
  return STATUS_SCORES[status];
}

export function getMessageOverallScore(message: Pick<ActionMessage, ActionMessageStatusField>): number {
  const totalScore = ACTION_MESSAGE_STATUS_CONFIG.reduce((sum, status) => {
    return sum + getStatusScore(message[status.field]);
  }, 0);

  return clampScore(totalScore / ACTION_MESSAGE_STATUS_CONFIG.length);
}

export function getOverallStatusBand(score: number): "Green" | "Yellow" | "Orange" | "Red" {
  const safeScore = clampScore(score);

  if (safeScore >= 75) {
    return "Green";
  }
  if (safeScore >= 50) {
    return "Yellow";
  }
  if (safeScore >= 25) {
    return "Orange";
  }
  return "Red";
}

export function getOverallRingColor(score: number): string {
  const safeScore = clampScore(score);

  if (safeScore >= 75) {
    return blendHexColor("#16ce79", "#3df58f", (safeScore - 75) / 25);
  }
  if (safeScore >= 50) {
    return blendHexColor("#f5cc19", "#16ce79", (safeScore - 50) / 25);
  }
  if (safeScore >= 25) {
    return blendHexColor("#ff9f1c", "#f5cc19", (safeScore - 25) / 25);
  }
  return blendHexColor("#ff3648", "#ff9f1c", safeScore / 25);
}
