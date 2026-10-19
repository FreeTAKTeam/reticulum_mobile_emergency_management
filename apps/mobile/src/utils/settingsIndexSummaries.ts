/** One-line presentation summaries for the Settings index rows and folded sections. */

export type HubModeValue = "Autonomous" | "SemiAutonomous" | "Connected";

const HUB_MODE_LABELS: Record<HubModeValue, string> = {
  Autonomous: "Autonomous",
  SemiAutonomous: "Semi-autonomous",
  Connected: "Connected",
};

export function hubModeLabel(mode: HubModeValue): string {
  return HUB_MODE_LABELS[mode] ?? mode;
}

export function identitySummary(callSign: string, householdName: string): string {
  const name = callSign.trim() || "No call sign";
  const household = householdName.trim();
  return household ? `${name} · Household "${household}"` : name;
}

export function teamsSummary(localCount: number, rchCount: number): string {
  return `${localCount} local · ${rchCount} from RCH`;
}

export function networkSummary(tcpCount: number, loraEnabled: boolean): string {
  return `${tcpCount} TCP · LoRa ${loraEnabled ? "on" : "off"}`;
}

export function routingSummary(mode: HubModeValue, hubIdentityHash: string): string {
  if (mode === "Autonomous") {
    return hubModeLabel(mode);
  }
  const hash = hubIdentityHash.trim();
  return hash
    ? `${hubModeLabel(mode)} · hub ${hash.slice(0, 8)}…`
    : `${hubModeLabel(mode)} · no hub selected`;
}

export type TelemetryLoopState = "idle" | "running" | "permission_denied" | "gps_unavailable" | "error";

export function telemetryStatusLabel(enabled: boolean, loopStatus: TelemetryLoopState): string {
  if (!enabled) return "Disabled";
  if (loopStatus === "permission_denied") return "Permission denied";
  if (loopStatus === "gps_unavailable") return "GPS unavailable";
  if (loopStatus === "error") return "Error";
  if (loopStatus === "running") return "Publishing";
  return "Idle";
}

export function locationSummary(
  enabled: boolean,
  publishIntervalSeconds: number,
  loopStatus: TelemetryLoopState,
): string {
  if (!enabled) return "Off";
  return `Every ${publishIntervalSeconds} s · ${telemetryStatusLabel(enabled, loopStatus).toLowerCase()}`;
}

export function mapSummary(layer: "base" | "satellite"): string {
  return layer === "satellite" ? "Satellite map" : "Base map";
}

export function sosSummary(
  enabled: boolean,
  triggers: { shake: boolean; powerButton: boolean; floatingButton: boolean },
): string {
  if (!enabled) return "Off";
  const names = [
    triggers.shake ? "Shake" : "",
    triggers.powerButton ? "power button" : "",
    triggers.floatingButton ? "floating button" : "",
  ].filter(Boolean);
  if (names.length === 0) return "On · manual only";
  const sentence = names.join(" + ");
  return `On · ${sentence.charAt(0).toUpperCase()}${sentence.slice(1)}`;
}

export function powerSummary(enabled: boolean, thresholdPercent: number): string {
  return enabled ? `On at ${thresholdPercent} %` : "Off";
}

export function pluginsSummary(installed: number, enabled: number): string {
  return installed === 0 ? "None installed" : `${installed} installed · ${enabled} enabled`;
}

export function nodeSummary(running: boolean, interfaceCount: number): string {
  if (!running) return "Stopped";
  return `Running · ${interfaceCount} interface${interfaceCount === 1 ? "" : "s"}`;
}

/** Folded section header line: the row summaries that say something, joined. */
export function foldSummary(parts: readonly string[]): string {
  return parts.map((part) => part.trim()).filter(Boolean).join(" · ");
}
