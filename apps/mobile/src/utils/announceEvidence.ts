export interface AnnounceEvidencePeer {
  appData?: string;
  latestAnnounceKind?: string;
  latestAnnounceClass?: string;
}

function parseCapabilityTokens(appData: string): string[] {
  return appData
    .split(/[,;\s]+/g)
    .map((token) => token.trim().toLowerCase())
    .filter((token) => token.length > 0 && !token.startsWith("name="));
}

export function announceHasEmergencyCapabilities(appData: string): boolean {
  const tokens = parseCapabilityTokens(appData);
  return tokens.includes("r3akt") && tokens.includes("emergencymessages");
}

export function peerHasRemAnnounceEvidence(peer: AnnounceEvidencePeer): boolean {
  return peer.latestAnnounceKind === "lxmf_delivery"
    && announceHasEmergencyCapabilities(peer.appData ?? "");
}
