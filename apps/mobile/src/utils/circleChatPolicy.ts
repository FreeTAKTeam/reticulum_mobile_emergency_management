import type { DiscoveredPeer, SavedPeer } from "../types/domain";
import { connectedPeerOptionsFor, type ConnectedPeerOption } from "./inboxPeerOptions";
import { normalizedValuesMatch } from "./stringValues";

export function innerCirclePeerOptions(
  reachablePeers: DiscoveredPeer[],
  savedPeers: SavedPeer[],
  savedDestinations: ReadonlySet<string>,
): ConnectedPeerOption[] {
  return connectedPeerOptionsFor(reachablePeers.filter((peer) => savedPeers.some((saved) =>
    saved.circleTier === "inner" && (
      normalizedValuesMatch(saved.destination, peer.destination)
      || normalizedValuesMatch(saved.destination, peer.lxmfDestinationHex ?? "")
    ),
  )), savedDestinations);
}

export function chatPolicyReason(
  saverActive: boolean,
  destination: string,
  savedPeer?: SavedPeer,
  hubIdentityHash = "",
): string {
  if (saverActive) {
    return "Power saver pauses ordinary chat and retry. SOS and permitted location updates remain available.";
  }
  // The configured hub is never a saved peer, but its chats must stay answerable.
  if (destination && savedPeer?.circleTier !== "inner" && !normalizedValuesMatch(destination, hubIdentityHash)) {
    return "Chat and exact location require a saved Inner Circle peer. Change this peer's tier in Peers.";
  }
  return "";
}
