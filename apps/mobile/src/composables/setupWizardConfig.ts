export type SetupWizardStepId =
  | "welcome"
  | "callsign"
  | "tcp"
  | "rnode"
  | "telemetry"
  | "permissions"
  | "sos"
  | "review";

export interface SetupWizardStep {
  id: SetupWizardStepId;
  label: string;
  title: string;
}

export const SETUP_STEPS: SetupWizardStep[] = [
  { id: "welcome", label: "Welcome", title: "Welcome to R.E.M." },
  { id: "callsign", label: "Call Sign", title: "Set your call sign" },
  { id: "permissions", label: "Permits", title: "Android permissions" },
  { id: "tcp", label: "TCP", title: "Choose TCP interfaces" },
  { id: "rnode", label: "LoRa", title: "Configure RNode LoRa" },
  { id: "telemetry", label: "Telemetry", title: "Telemetry sharing" },
  { id: "sos", label: "SOS", title: "SOS emergency access" },
  { id: "review", label: "Review", title: "Review setup" },
];

export const USB_BOND_POLL_ATTEMPTS = 15;
export const USB_BOND_POLL_DELAY_MS = 2_000;

const DEFAULT_TELEMETRY_PUBLISH_INTERVAL_SECONDS = 360;

export function normalizeWizardTelemetryPublishIntervalSeconds(
  value: number | string | undefined | null,
): number {
  const parsed = Math.trunc(Number(value));
  return Number.isFinite(parsed) ? Math.max(1, parsed) : DEFAULT_TELEMETRY_PUBLISH_INTERVAL_SECONDS;
}

export function normalizeWizardTcpEndpoint(value: string): string | undefined {
  const candidate = value.trim();
  if (!candidate) {
    return undefined;
  }

  if (candidate.startsWith("[")) {
    const ipv6Match = candidate.match(/^\[[^\]]+\]:(\d{1,5})$/);
    if (!ipv6Match) {
      return undefined;
    }
    const port = Number(ipv6Match[1]);
    return Number.isInteger(port) && port >= 1 && port <= 65535 ? candidate : undefined;
  }

  const separatorIndex = candidate.lastIndexOf(":");
  if (separatorIndex <= 0 || separatorIndex === candidate.length - 1) {
    return undefined;
  }

  const host = candidate.slice(0, separatorIndex).trim();
  const port = Number(candidate.slice(separatorIndex + 1).trim());
  if (!host || !Number.isInteger(port) || port < 1 || port > 65535) {
    return undefined;
  }
  return `${host}:${port}`;
}
