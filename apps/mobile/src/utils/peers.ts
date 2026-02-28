import type { PeerListV1, PeerListV1Peer, SavedPeer } from "../types/domain";

const DEST_HEX_REGEX = /^[0-9a-f]{32}$/i;
const CONTROL_CHAR_REGEX = /[\u0000-\u001f\u007f]+/g;
const DISPLAY_NAME_TOKEN_PREFIX = "name=";
const MAX_DISPLAY_NAME_LENGTH = 64;

export function normalizeDestinationHex(value: string): string {
  return value.trim().toLowerCase();
}

export function isValidDestinationHex(value: string): boolean {
  return DEST_HEX_REGEX.test(value.trim());
}

function tokenizeAnnounceAppData(appData: string): string[] {
  return appData
    .split(/[,;\s]+/g)
    .map((token) => token.trim())
    .filter((token) => token.length > 0);
}

function isDisplayNameToken(token: string): boolean {
  return token.toLowerCase().startsWith(DISPLAY_NAME_TOKEN_PREFIX);
}

export function normalizeDisplayName(value: string): string | undefined {
  const sanitized = value
    .replace(CONTROL_CHAR_REGEX, " ")
    .replace(/\s+/g, " ")
    .trim();
  if (!sanitized) {
    return undefined;
  }
  return sanitized.slice(0, MAX_DISPLAY_NAME_LENGTH);
}

export function extractAnnouncedName(appData: string): string | undefined {
  const nameToken = tokenizeAnnounceAppData(appData).find((token) => isDisplayNameToken(token));
  if (!nameToken) {
    return undefined;
  }

  const encodedName = nameToken.slice(DISPLAY_NAME_TOKEN_PREFIX.length);
  if (!encodedName) {
    return undefined;
  }

  try {
    return normalizeDisplayName(decodeURIComponent(encodedName));
  } catch {
    return undefined;
  }
}

export function extractAnnounceCapabilityText(appData: string): string {
  return tokenizeAnnounceAppData(appData)
    .filter((token) => !isDisplayNameToken(token))
    .join(",");
}

export function formatAnnounceAppData(
  capabilityText: string,
  displayName?: string,
): string {
  const normalizedCapabilityText = extractAnnounceCapabilityText(capabilityText);
  const normalizedDisplayName = normalizeDisplayName(displayName ?? "");
  if (!normalizedDisplayName) {
    return normalizedCapabilityText;
  }
  if (!normalizedCapabilityText) {
    return `${DISPLAY_NAME_TOKEN_PREFIX}${encodeURIComponent(normalizedDisplayName)}`;
  }
  return `${normalizedCapabilityText};${DISPLAY_NAME_TOKEN_PREFIX}${encodeURIComponent(normalizedDisplayName)}`;
}

export function parseCapabilityTokens(appData: string): string[] {
  return tokenizeAnnounceAppData(appData)
    .filter((token) => !isDisplayNameToken(token))
    .map((token) => token.toLowerCase())
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
