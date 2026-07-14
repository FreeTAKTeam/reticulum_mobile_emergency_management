import type { EamReadinessSummaryRecord } from "@reticulum/node-client";

import type { ActionMessage, EamStatus, EamTeamSummary, EamWireStatus } from "../types/domain";

type TeamStatusBuckets = Partial<Record<EamWireStatus, number>>;

type StatusField =
  | "securityStatus"
  | "capabilityStatus"
  | "preparednessStatus"
  | "medicalStatus"
  | "mobilityStatus"
  | "commsStatus";

const STATUS_FIELDS: Array<{ field: StatusField; label: string }> = [
  { field: "securityStatus", label: "Security" },
  { field: "capabilityStatus", label: "Capability" },
  { field: "preparednessStatus", label: "Preparedness" },
  { field: "medicalStatus", label: "Medical" },
  { field: "mobilityStatus", label: "Mobility" },
  { field: "commsStatus", label: "Comms" },
];

function statusScore(status: EamStatus): number {
  if (status === "Green") {
    return 100;
  }
  if (status === "Yellow") {
    return 50;
  }
  if (status === "Red") {
    return 25;
  }
  return 0;
}

function readinessBand(score: number): string {
  if (score >= 75) {
    return "Green";
  }
  if (score >= 50) {
    return "Yellow";
  }
  if (score >= 25) {
    return "Orange";
  }
  return "Red";
}

function parseHexColor(value: string): [number, number, number] {
  const hex = value.replace(/^#/, "");
  return [
    Number.parseInt(hex.slice(0, 2), 16),
    Number.parseInt(hex.slice(2, 4), 16),
    Number.parseInt(hex.slice(4, 6), 16),
  ];
}

function blendHexColor(start: string, end: string, ratio: number): string {
  const safeRatio = Math.max(0, Math.min(1, ratio));
  const startChannels = parseHexColor(start);
  const endChannels = parseHexColor(end);
  const channels = startChannels.map((startValue, index) =>
    Math.round(startValue + ((endChannels[index] - startValue) * safeRatio)),
  );
  return `#${channels.map((channel) => channel.toString(16).padStart(2, "0")).join("")}`;
}

function readinessRingColor(score: number): string {
  const safeScore = Math.max(0, Math.min(100, score));
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

function averageScore(values: number[]): number {
  if (values.length === 0) {
    return 0;
  }
  return Math.round(values.reduce((total, value) => total + value, 0) / values.length);
}

export function buildWebEamReadinessSummary(
  records: ActionMessage[],
): EamReadinessSummaryRecord {
  const activeRecords = records.filter((record) => !record.deletedAt);
  const statusMetrics = STATUS_FIELDS.map(({ field, label }) => {
    const score = averageScore(activeRecords.map((record) => statusScore(record[field])));
    return {
      field,
      label,
      score,
      band: readinessBand(score),
      ringColor: readinessRingColor(score),
    };
  });
  const messages = activeRecords.map((record) => {
    const overallScore = averageScore(
      STATUS_FIELDS.map(({ field }) => statusScore(record[field])),
    );
    return {
      callsign: record.callsign,
      overallScore,
      overallBand: readinessBand(overallScore),
      overallRingColor: readinessRingColor(overallScore),
    };
  });

  return {
    activeTotal: activeRecords.length,
    updatedAt: records.reduce((latest, record) => Math.max(latest, record.updatedAt), 0),
    statusMetrics,
    messages,
  };
}

export function computeWebEamTeamSummary(
  messages: ActionMessage[],
  teamUid: string,
): EamTeamSummary {
  const teamMessages = messages.filter(
    (message) => message.teamUid === teamUid && !message.deletedAt,
  );
  const byStatus: TeamStatusBuckets = {};
  for (const message of teamMessages) {
    const status = message.overallStatus;
    if (status) {
      byStatus[status] = (byStatus[status] ?? 0) + 1;
    }
  }
  const overallStatus = byStatus.Red
    ? "Red"
    : byStatus.Yellow
      ? "Yellow"
      : byStatus.Green
        ? "Green"
        : undefined;

  return {
    team_uid: teamUid,
    total: teamMessages.length,
    active_total: teamMessages.length,
    deleted_total: messages.filter((message) => message.teamUid === teamUid && Boolean(message.deletedAt)).length,
    overall_status: overallStatus,
    by_status: byStatus,
    updated_at: new Date().toISOString(),
  };
}
