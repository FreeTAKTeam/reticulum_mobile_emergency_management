import type { AnnounceRecord, NodeStatus, SyncStatus } from "@reticulum/node-client";
import { computed, type Ref } from "vue";

import type { HubRegistryBootstrapProfile } from "../services/hubRegistryBootstrap";
import type {
  DiscoveredPeer,
  HubDirectorySnapshot,
  PeerConnectionState,
  SavedPeer,
} from "../types/domain";
import { isValidDestinationHex, normalizeDestinationHex } from "../utils/peers";
import { statusHasRuntimeReceiveReadiness } from "../utils/startupInterfaces";
import { storeRemovedPeerDestinations } from "./nodeSettingsModel";
import {
  PEER_ONLINE_FRESHNESS_MS,
  type EventPeerRoute,
  type HubAnnounceCandidate,
  type HubRegistrationSnapshot,
  activePropagationNodeHex,
  asTrimmedString,
  hasActualRemAnnounce,
  nowMs,
  peerSortRank,
  shouldDisplayDiscoveredPeer,
} from "./nodeStoreCore";

interface NodePeerSelectorsContext {
  announceByDestination: Record<string, AnnounceRecord>;
  currentHubBootstrapProfile: () => HubRegistryBootstrapProfile | null;
  discoveredByDestination: Record<string, DiscoveredPeer>;
  hubDirectorySnapshot: Ref<HubDirectorySnapshot | null>;
  hubRegistration: HubRegistrationSnapshot;
  isLocalPeer: (peer: Pick<DiscoveredPeer, "destination" | "identityHex">) => boolean;
  presenceNow: Ref<number>;
  readinessError: Ref<string>;
  removedByDestination: Record<string, number>;
  savedByDestination: Record<string, SavedPeer>;
  status: Ref<NodeStatus>;
  syncStatus: Ref<SyncStatus>;
}

export function createNodePeerSelectors(context: NodePeerSelectorsContext) {
  const {
    announceByDestination,
    currentHubBootstrapProfile,
    discoveredByDestination,
    hubDirectorySnapshot,
    hubRegistration,
    isLocalPeer,
    presenceNow,
    readinessError,
    removedByDestination,
    savedByDestination,
    status,
    syncStatus,
  } = context;

  function hasFreshPresence(lastSeenAt?: number): boolean {
    return typeof lastSeenAt === "number"
      && Number.isFinite(lastSeenAt)
      && (presenceNow.value - lastSeenAt) <= PEER_ONLINE_FRESHNESS_MS;
  }

  function peerPresenceTimestamp(peer: Pick<DiscoveredPeer, "lastSeenAt">): number | undefined {
    const seenAt = peer.lastSeenAt ?? 0;
    return seenAt > 0 ? seenAt : undefined;
  }

  function peerCachedPresenceTimestamp(
    peer: Pick<DiscoveredPeer, "announceLastSeenAt" | "lxmfLastSeenAt" | "lastSeenAt">,
  ): number | undefined {
    const announceSeenAt = typeof peer.announceLastSeenAt === "number" ? peer.announceLastSeenAt : 0;
    const lxmfSeenAt = typeof peer.lxmfLastSeenAt === "number" ? peer.lxmfLastSeenAt : 0;
    const seenAt = Math.max(announceSeenAt, lxmfSeenAt, peer.lastSeenAt ?? 0);
    return seenAt > 0 ? seenAt : undefined;
  }

  function peerDisplayState(peer: Pick<DiscoveredPeer, "state">): PeerConnectionState {
    return peer.state;
  }

  function peerIsSaved(
    peer: Pick<DiscoveredPeer, "destination" | "saved">,
    savedDestinationSet: Set<string>,
  ): boolean {
    return savedDestinationSet.has(peer.destination) || peer.saved;
  }

  function peerPresenceState(
    peer: Pick<DiscoveredPeer, "announceLastSeenAt" | "lxmfLastSeenAt" | "lastSeenAt">,
  ): "online" | "offline" {
    return hasFreshPresence(peerCachedPresenceTimestamp(peer)) ? "online" : "offline";
  }

  function peerHasKnownLxmfRoute(
    peer: Pick<DiscoveredPeer, "destination" | "lxmfDestinationHex">,
  ): boolean {
    const appDestinationHex = normalizeDestinationHex(peer.destination);
    const lxmfDestinationHex = normalizeDestinationHex(peer.lxmfDestinationHex ?? "");
    return isValidDestinationHex(appDestinationHex) && isValidDestinationHex(lxmfDestinationHex);
  }

  function peerByAnyKnownDestination(
    peers: Record<string, DiscoveredPeer>,
    destinationRaw: string,
  ): DiscoveredPeer | undefined {
    const destinationHex = normalizeDestinationHex(destinationRaw);
    if (!isValidDestinationHex(destinationHex)) {
      return undefined;
    }
    return Object.values(peers).find((peer) =>
      destinationHex === normalizeDestinationHex(peer.destination)
      || destinationHex === normalizeDestinationHex(peer.lxmfDestinationHex ?? "")
      || destinationHex === normalizeDestinationHex(peer.identityHex ?? ""),
    );
  }

  function knownDestinationsForPeer(
    destinationRaw: string,
    peer?: Pick<DiscoveredPeer, "destination" | "lxmfDestinationHex" | "identityHex">,
  ): string[] {
    const destinations = [destinationRaw, peer?.destination, peer?.lxmfDestinationHex, peer?.identityHex]
      .map((value) => normalizeDestinationHex(value ?? ""))
      .filter(isValidDestinationHex);
    return [...new Set(destinations)];
  }

  function peerIsRemoved(
    peer: Pick<DiscoveredPeer, "destination" | "lxmfDestinationHex" | "identityHex">,
  ): boolean {
    return knownDestinationsForPeer(peer.destination, peer).some(
      (destination) => removedByDestination[destination] !== undefined,
    );
  }

  function markPeerRemoved(destinationRaw: string, peer?: DiscoveredPeer): string[] {
    const destinations = knownDestinationsForPeer(destinationRaw, peer);
    const removedAt = nowMs();
    for (const destination of destinations) {
      removedByDestination[destination] = removedAt;
    }
    storeRemovedPeerDestinations({ ...removedByDestination });
    return destinations;
  }

  function clearPeerRemoved(destinationRaw: string, peer?: DiscoveredPeer): void {
    for (const destination of knownDestinationsForPeer(destinationRaw, peer)) {
      delete removedByDestination[destination];
    }
    storeRemovedPeerDestinations({ ...removedByDestination });
  }

  const discoveredPeers = computed(() =>
    Object.values(discoveredByDestination)
      .filter(shouldDisplayDiscoveredPeer)
      .filter((peer) => !peerIsRemoved(peer))
      .filter((peer) => !isLocalPeer(peer))
      .sort((a, b) => peerSortRank(b) - peerSortRank(a) || b.lastSeenAt - a.lastSeenAt),
  );
  const allPeers = discoveredPeers;

  const remAnnouncedPeers = computed(() =>
    Object.values(discoveredByDestination)
      .filter((peer) => !isLocalPeer(peer))
      .filter((peer) => !peerIsRemoved(peer))
      .filter(hasActualRemAnnounce)
      .sort((a, b) => b.lastSeenAt - a.lastSeenAt),
  );

  const autoFanoutPeers = computed(() =>
    Object.values(discoveredByDestination)
      .filter((peer) => !isLocalPeer(peer))
      .filter((peer) => peerIsSaved(peer, savedDestinations.value))
      .filter(peerHasKnownLxmfRoute)
      .sort((a, b) => b.lastSeenAt - a.lastSeenAt),
  );

  const bestPropagationNodeHex = computed(() => activePropagationNodeHex(syncStatus.value));
  const propagationEligibleEventPeerRoutes = computed<EventPeerRoute[]>(() =>
    (!bestPropagationNodeHex.value ? [] : autoFanoutPeers.value)
      .filter((peer) => !peer.activeLink)
      .sort((a, b) => b.lastSeenAt - a.lastSeenAt)
      .map((peer) => ({
        appDestinationHex: peer.destination,
        lxmfDestinationHex: peer.lxmfDestinationHex!,
        identityHex: peer.identityHex,
        label: peer.label,
        announcedName: peer.announcedName,
        sendMode: "PropagationOnly",
      })),
  );

  function savedPeerLastSeenAt(peer: SavedPeer): number {
    const discovered = peerByAnyKnownDestination(discoveredByDestination, peer.destination);
    return discovered ? peerCachedPresenceTimestamp(discovered) ?? 0 : 0;
  }

  const savedPeers = computed(() =>
    Object.values(savedByDestination).sort((a, b) =>
      savedPeerLastSeenAt(b) - savedPeerLastSeenAt(a)
      || b.savedAt - a.savedAt
      || a.destination.localeCompare(b.destination),
    ),
  );
  const savedDestinations = computed(() => new Set(savedPeers.value.map((peer) => peer.destination)));
  const savedVisiblePeers = computed(() =>
    discoveredPeers.value.filter((peer) => peerIsSaved(peer, savedDestinations.value)),
  );
  const connectedPeers = computed(() => savedVisiblePeers.value.filter((peer) => peer.activeLink));
  const reachablePeers = computed(() =>
    savedVisiblePeers.value.filter((peer) => hasFreshPresence(peerCachedPresenceTimestamp(peer))),
  );
  const connectedDestinations = computed(() => connectedPeers.value.map((peer) => peer.destination));
  const intentionalPeerDestinations = computed(() =>
    savedVisiblePeers.value.map((peer) => peer.destination),
  );
  const connectedLinkDestinations = computed(() => connectedPeers.value.map((peer) => peer.destination));
  const connectedEventPeerRoutes = computed<EventPeerRoute[]>(() =>
    connectedPeers.value
      .filter(peerHasKnownLxmfRoute)
      .map((peer) => ({
        appDestinationHex: peer.destination,
        lxmfDestinationHex: peer.lxmfDestinationHex!,
        identityHex: peer.identityHex,
        label: peer.label,
        announcedName: peer.announcedName,
        sendMode: "Auto",
      })),
  );

  const visiblePeerCount = computed(() => discoveredPeers.value.length);
  const savedPeerCount = computed(() => savedPeers.value.length);
  const connectedPeerCount = computed(() => connectedPeers.value.length);
  const reachablePeerCount = computed(() => reachablePeers.value.length);
  const hubDirectoryPeers = computed(() => hubDirectorySnapshot.value?.items ?? []);
  const effectiveConnectedMode = computed(() => Boolean(hubDirectorySnapshot.value?.effectiveConnectedMode));
  const hubAnnounceCandidates = computed<HubAnnounceCandidate[]>(() => {
    const byIdentity = new Map<string, HubAnnounceCandidate & { receivedAtMs: number }>();
    for (const announce of Object.values(announceByDestination)) {
      if (announce.announceClass !== "RchHubServer") {
        continue;
      }
      const identity = isValidDestinationHex(announce.identityHex)
        ? announce.identityHex
        : announce.destinationHex;
      const candidate = {
        destination: identity,
        label: announce.displayName || identity,
        receivedAtMs: announce.receivedAtMs,
      };
      const existing = byIdentity.get(identity);
      if (!existing || existing.receivedAtMs < announce.receivedAtMs) {
        byIdentity.set(identity, candidate);
      }
    }
    return [...byIdentity.values()]
      .map(({ destination, label }) => ({ destination, label }))
      .sort((left, right) => left.label.localeCompare(right.label)
        || left.destination.localeCompare(right.destination));
  });

  const readinessErrorMessage = computed(() => asTrimmedString(readinessError.value));
  const ready = computed(() => status.value.running && statusHasRuntimeReceiveReadiness(status.value));
  const hubBootstrapProfile = computed(() => currentHubBootstrapProfile());
  const hubRegistrationReady = computed(
    () => hubRegistration.status === "ready" && Boolean(hubRegistration.linkage),
  );
  const hubRegistrationPending = computed(() => hubRegistration.status === "pending");
  const hubRegistrationSummary = computed(() => {
    const lastHubError = asTrimmedString(hubRegistration.lastError);
    switch (hubRegistration.status) {
      case "disabled":
        return "Hub sync disabled";
      case "ready":
        return hubRegistration.linkage
          ? `Ready | team=${hubRegistration.linkage.teamUid.slice(0, 10)}... member=${hubRegistration.linkage.teamMemberUid.slice(0, 10)}...`
          : "Hub registration ready";
      case "error":
        return lastHubError ? `Error | ${lastHubError}` : "Hub registration error";
      case "pending":
      default:
        return lastHubError ? `Pending | ${lastHubError}` : "Pending hub registration";
    }
  });

  return {
    allPeers,
    bestPropagationNodeHex,
    clearPeerRemoved,
    connectedDestinations,
    connectedEventPeerRoutes,
    connectedLinkDestinations,
    connectedPeerCount,
    connectedPeers,
    discoveredPeers,
    effectiveConnectedMode,
    hubAnnounceCandidates,
    hubBootstrapProfile,
    hubDirectoryPeers,
    hubRegistrationPending,
    hubRegistrationReady,
    hubRegistrationSummary,
    intentionalPeerDestinations,
    markPeerRemoved,
    peerByAnyKnownDestination,
    peerCachedPresenceTimestamp,
    peerDisplayState,
    peerPresenceState,
    peerPresenceTimestamp,
    propagationEligibleEventPeerRoutes,
    reachablePeerCount,
    reachablePeers,
    readinessErrorMessage,
    ready,
    remAnnouncedPeers,
    savedDestinations,
    savedPeerCount,
    savedPeers,
    savedVisiblePeers,
    visiblePeerCount,
  };
}
