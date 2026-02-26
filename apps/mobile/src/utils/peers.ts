import type { PeerListV1, PeerListV1Peer, SavedPeer } from "../types/domain";

const DEST_HEX_REGEX = /^[0-9a-f]{32}$/i;

export function normalizeDestinationHex(value: string): string {
  return value.trim().toLowerCase();
}

export function isValidDestinationHex(value: string): boolean {
  return DEST_HEX_REGEX.test(value.trim());
}

export function parseCapabilityTokens(appData: string): string[] {
  return appData
    .split(/[,;\s]+/g)
    .map((token) => token.trim().toLowerCase())
    .filter((token) => token.length > 0);
}

export function matchesEmergencyCapabilities(appData: string): boolean {
  const tokens = parseCapabilityTokens(appData);
  return tokens.includes("r3akt") && tokens.includes("emergencymessages");
}

export function createPeerListV1(peers: SavedPeer[]): PeerListV1 {
  const normalizedPeers = peers
    .map((peer) => ({
      destination: normalizeDestinationHex(peer.destination),
      label: peer.label?.trim() || undefined,
    }))
    .filter((peer) => isValidDestinationHex(peer.destination));

  return {
    version: 1,
    generatedAt: new Date().toISOString(),
    capabilities: ["R3AKT", "EMergencyMessages"],
    peers: normalizedPeers,
  };
}

export interface ParsedPeerList {
  peerList: PeerListV1;
  warnings: string[];
}

function normalizePeer(entry: unknown): PeerListV1Peer | null {
  if (!entry || typeof entry !== "object") {
    return null;
  }

  const rawDestination = String(
    (entry as { destination?: unknown }).destination ?? "",
  );
  const destination = normalizeDestinationHex(rawDestination);
  if (!isValidDestinationHex(destination)) {
    return null;
  }

  const rawLabel = (entry as { label?: unknown }).label;
  const label =
    typeof rawLabel === "string" && rawLabel.trim().length > 0
      ? rawLabel.trim()
      : undefined;

  return { destination, label };
}

export function parsePeerListV1(jsonText: string): ParsedPeerList {
  const raw = JSON.parse(jsonText) as Partial<PeerListV1>;
  if (raw.version !== 1) {
    throw new Error("Unsupported peer list version. Expected version=1.");
  }

  if (!Array.isArray(raw.peers)) {
    throw new Error("Peer list payload is invalid: peers array is missing.");
  }

  const normalizedPeers = raw.peers
    .map((entry) => normalizePeer(entry))
    .filter((entry): entry is PeerListV1Peer => entry !== null);

  const deduplicated = new Map<string, PeerListV1Peer>();
  for (const peer of normalizedPeers) {
    deduplicated.set(peer.destination, peer);
  }

  const warnings: string[] = [];
  if (!Array.isArray(raw.capabilities)) {
    warnings.push("Capabilities field is missing. Import continues.");
  } else {
    const capabilityTokens = raw.capabilities.map((cap) => cap.toLowerCase());
    if (
      !capabilityTokens.includes("r3akt") ||
      !capabilityTokens.includes("emergencymessages")
    ) {
      warnings.push(
        "Capabilities do not match R3AKT/EMergencyMessages. Destinations imported as provided.",
      );
    }
  }

  return {
    peerList: {
      version: 1,
      generatedAt:
        typeof raw.generatedAt === "string" && raw.generatedAt.length > 0
          ? raw.generatedAt
          : new Date().toISOString(),
      capabilities: ["R3AKT", "EMergencyMessages"],
      peers: [...deduplicated.values()],
    },
    warnings,
  };
}
