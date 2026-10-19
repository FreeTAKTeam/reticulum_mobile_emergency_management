/**
 * Settings fields that are part of the native node configuration. Changing one
 * while the node runs only takes effect after a node restart; the runtime store
 * decides whether a restart is pending, this only names what changed.
 */
export interface RestartRelevantSettings {
  displayName: string;
  announceCapabilities: string;
  announceIntervalSeconds: number;
  tcpClients: string[];
  broadcast: boolean;
  transportNodeEnabled: boolean;
  telemetry: { staleAfterMinutes: number };
  hub: {
    mode: string;
    identityHash: string;
    apiBaseUrl: string;
    apiKey: string;
    refreshIntervalSeconds: number;
  };
  rnode: object;
}

const RESTART_FIELDS: Array<[string, (settings: RestartRelevantSettings) => unknown]> = [
  ["call sign", (settings) => settings.displayName],
  [
    "announce settings",
    (settings) => [settings.announceCapabilities, settings.announceIntervalSeconds],
  ],
  ["TCP interfaces", (settings) => settings.tcpClients],
  ["LoRa interface", (settings) => settings.rnode],
  ["broadcast", (settings) => settings.broadcast],
  ["transport forwarding", (settings) => settings.transportNodeEnabled],
  ["stale-after time", (settings) => settings.telemetry.staleAfterMinutes],
  ["hub routing", (settings) => settings.hub],
];

/** Plain-language names of the restart-relevant fields that differ between two settings snapshots. */
export function changedRestartFields(
  before: RestartRelevantSettings,
  after: RestartRelevantSettings,
): string[] {
  return RESTART_FIELDS
    .filter(([, pick]) => JSON.stringify(pick(before)) !== JSON.stringify(pick(after)))
    .map(([label]) => label);
}

/** Banner text for a pending restart; reasons may be empty after an app relaunch. */
export function restartBannerText(reasons: readonly string[]): string {
  return reasons.length > 0
    ? `Restart node to apply: ${reasons.join(", ")}`
    : "Restart node to apply saved changes";
}
