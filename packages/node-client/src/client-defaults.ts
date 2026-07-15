import type { NodeConfig, RuntimeReadinessSnapshot, SavedPeerRecord } from "./contracts";

const GREEK_CALLSIGN_PREFIXES = [
  "Alpha",
  "Beta",
  "Gamma",
  "Delta",
  "Epsilon",
  "Zeta",
  "Eta",
  "Theta",
  "Iota",
  "Kappa",
  "Lambda",
  "Mu",
  "Nu",
  "Xi",
  "Omicron",
  "Pi",
  "Rho",
  "Sigma",
  "Tau",
  "Upsilon",
  "Phi",
  "Chi",
  "Psi",
  "Omega",
] as const;

export function generateDefaultCallSign(): string {
  const prefix = GREEK_CALLSIGN_PREFIXES[Math.floor(Math.random() * GREEK_CALLSIGN_PREFIXES.length)];
  const suffix = String(Math.floor(Math.random() * 999) + 1).padStart(3, "0");
  return `${prefix}${suffix}`;
}

export const DEFAULT_NODE_CONFIG: NodeConfig = {
  name: generateDefaultCallSign(),
  tcpClients: [],
  broadcast: true,
  transportNodeEnabled: true,
  announceIntervalSeconds: 1800,
  staleAfterMinutes: 30,
  announceCapabilities: "R3AKT,EMergencyMessages",
  hubMode: "Autonomous",
  hubRefreshIntervalSeconds: 3600,
  rnode: {
    enabled: false,
    connectionMode: "ble",
    peripheralId: "",
    displayName: "",
    region: "US915",
    profile: "REM-LF-RURAL-v1",
  },
};

export function browserRuntimeReadiness(running: boolean): RuntimeReadinessSnapshot {
  return {
    state: running ? "Ready" : "Pending",
    interfaces: [
      {
        id: "local",
        label: "Reticulum Net",
        state: running ? "Ready" : "Pending",
        detail: running ? "Browser runtime is ready" : "Browser runtime is starting",
      },
    ],
  };
}

export function randomHex32(): string {
  const chars = "0123456789abcdef";
  let out = "";
  for (let i = 0; i < 32; i += 1) {
    out += chars[Math.floor(Math.random() * chars.length)];
  }
  return out;
}

export function countConnectedSavedPeers(
  connected: Set<string>,
  savedPeers: Map<string, SavedPeerRecord>,
): number {
  return [...connected].filter((destination) => savedPeers.has(destination)).length;
}
