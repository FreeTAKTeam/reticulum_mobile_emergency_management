import type { DiscoveredPeer } from "../types/domain";
import { normalizedValuesMatch, safeLower, safeTrim } from "./stringValues";

export interface ConnectedPeerOption {
  value: string;
  displayName: string;
}

export function connectedPeerOptionsFor(
  peers: readonly DiscoveredPeer[],
  savedDestinations: ReadonlySet<string>,
): ConnectedPeerOption[] {
  const seen = new Set<string>();
  return peers
    .filter((peer) => savedDestinations.has(peer.destination))
    .map((peer) => {
      const value = safeTrim(peer.lxmfDestinationHex) || safeTrim(peer.destination);
      const baseName = safeTrim(peer.announcedName) || safeTrim(peer.label) || value;
      const displayName = peer.activeLink ? `${baseName} (Connected)` : `${baseName} (Reachable)`;
      return { value, displayName };
    })
    .filter((option) => {
      const normalizedValue = safeLower(option.value);
      if (!normalizedValue || seen.has(normalizedValue)) return false;
      seen.add(normalizedValue);
      return true;
    })
    .sort((left, right) => left.displayName.localeCompare(right.displayName));
}

export function withSelectedPeerOption(
  options: readonly ConnectedPeerOption[],
  selected: ConnectedPeerOption | null,
): ConnectedPeerOption[] {
  const next = [...options];
  if (selected && !next.some((option) => normalizedValuesMatch(option.value, selected.value))) {
    next.unshift(selected);
  }
  return next;
}

export function filterPeerOptions(
  options: readonly ConnectedPeerOption[],
  query: string,
): ConnectedPeerOption[] {
  const normalizedQuery = safeLower(query);
  if (!normalizedQuery) return [...options];
  return options.filter((option) =>
    safeLower(option.value).includes(normalizedQuery)
    || safeLower(option.displayName).includes(normalizedQuery));
}
