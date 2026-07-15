import type { DiscoveredPeer, SavedPeer } from "../types/domain";

export function discoveredPeerMatchesQuery(peer: DiscoveredPeer, query: string): boolean {
  return (
    peer.destination.includes(query) ||
    (peer.label ?? "").toLowerCase().includes(query) ||
    (peer.announcedName ?? "").toLowerCase().includes(query) ||
    (peer.appData ?? "").toLowerCase().includes(query)
  );
}

export function savedPeerMatchesQuery(
  peer: SavedPeer,
  query: string,
  announcedName?: string,
): boolean {
  return (
    peer.destination.includes(query) ||
    (peer.label ?? "").toLowerCase().includes(query) ||
    (announcedName ?? "").toLowerCase().includes(query)
  );
}
