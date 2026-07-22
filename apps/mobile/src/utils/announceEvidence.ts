import { matchesEmergencyCapabilities } from "./peers";

export interface AnnounceEvidencePeer {
  appData?: string;
  latestAnnounceKind?: string;
  latestAnnounceClass?: string;
}

export function announceHasEmergencyCapabilities(appData: string): boolean {
  return matchesEmergencyCapabilities(appData);
}

export function peerHasRemAnnounceEvidence(peer: AnnounceEvidencePeer): boolean {
  return peer.latestAnnounceKind === "lxmf_delivery"
    && announceHasEmergencyCapabilities(peer.appData ?? "");
}
