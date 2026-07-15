import {
  type AnnounceReceivedEvent,
  type AnnounceRecord,
  type NodeStatus,
  type PeerChangedEvent,
  type PeerRecord,
} from "@reticulum/node-client";
import type { Ref } from "vue";

import type {
  DiscoveredPeer,
  HubDirectorySnapshot,
  PeerConnectionState,
  SavedPeer,
} from "../types/domain";
import { peerHasRemAnnounceEvidence } from "../utils/announceEvidence";
import {
  isValidDestinationHex,
  normalizeDestinationHex,
} from "../utils/peers";
import {
  nowMs,
  sleep,
  toUiPeerState,
} from "./nodeStoreCore";

interface NodePeerContext {
  announceByDestination: Record<string, AnnounceRecord>;
  appDestinationByIdentity: Record<string, string>;
  discoveredByDestination: Record<string, DiscoveredPeer>;
  hubDirectorySnapshot: Ref<HubDirectorySnapshot | null>;
  lastHubRefreshAt: Ref<number>;
  lxmfDestinationByIdentity: Record<string, string>;
  nativeSavedPeerForCanonicalDestination: (
    canonicalDestination: string,
    identityHex: string | undefined,
    nativeSaved: boolean,
    displayName?: string,
  ) => SavedPeer | undefined;
  refreshSavedPeerProfile: (destinationRaw: string, reason: string) => void;
  refreshMessagingState: () => Promise<void>;
  savedByDestination: Record<string, SavedPeer>;
  status: Ref<NodeStatus>;
}

export function createNodePeerController(context: NodePeerContext) {
  const {
    announceByDestination,
    appDestinationByIdentity,
    discoveredByDestination,
    hubDirectorySnapshot,
    lastHubRefreshAt,
    lxmfDestinationByIdentity,
    nativeSavedPeerForCanonicalDestination,
    refreshSavedPeerProfile,
    refreshMessagingState,
    savedByDestination,
    status,
  } = context;

  function upsertDiscovered(
    destinationRaw: string,
    patch: Partial<DiscoveredPeer>,
    source?: "announce" | "hub" | "import",
  ): void {
    const destination = normalizeDestinationHex(destinationRaw);
    if (!isValidDestinationHex(destination)) {
      return;
    }

    const existing = discoveredByDestination[destination];
    const sources = existing ? [...existing.sources] : [];
    if (source && !sources.includes(source)) {
      sources.push(source);
    }

    const base: DiscoveredPeer = existing ?? {
      destination,
      lastSeenAt: nowMs(),
      sources,
      state: "disconnected",
      saved: false,
      stale: false,
      activeLink: false,
    };

    discoveredByDestination[destination] = {
      ...base,
      ...patch,
      destination,
      sources,
      identityHex: patch.identityHex ?? base.identityHex,
      lxmfDestinationHex: patch.lxmfDestinationHex ?? base.lxmfDestinationHex,
      announceLastSeenAt: patch.announceLastSeenAt ?? base.announceLastSeenAt,
      lxmfLastSeenAt: patch.lxmfLastSeenAt ?? base.lxmfLastSeenAt,
      announcedName: patch.announcedName ?? base.announcedName,
      label: patch.label ?? base.label,
      appData: patch.appData ?? base.appData,
      latestAnnounceKind: patch.latestAnnounceKind ?? base.latestAnnounceKind,
      latestAnnounceClass: patch.latestAnnounceClass ?? base.latestAnnounceClass,
      hops: patch.hops ?? base.hops,
      interfaceHex: patch.interfaceHex ?? base.interfaceHex,
      saved: patch.saved ?? base.saved,
      stale: patch.stale ?? base.stale,
      activeLink: patch.activeLink ?? base.activeLink,
      lastError: Object.prototype.hasOwnProperty.call(patch, "lastError")
        ? patch.lastError
        : base.lastError,
      lastResolutionError: Object.prototype.hasOwnProperty.call(patch, "lastResolutionError")
        ? patch.lastResolutionError
        : base.lastResolutionError,
      lastResolutionAttemptAt: patch.lastResolutionAttemptAt ?? base.lastResolutionAttemptAt,
      lastSeenAt: patch.lastSeenAt ?? base.lastSeenAt,
    };
  }

  function upsertNativeAnnounceRecord(
    record: AnnounceReceivedEvent | AnnounceRecord,
  ): void {
    const destination = normalizeDestinationHex(record.destinationHex);
    if (!isValidDestinationHex(destination)) {
      return;
    }
    const existing = announceByDestination[destination];
    if (existing && existing.receivedAtMs > record.receivedAtMs) {
      return;
    }
    announceByDestination[destination] = {
      destinationHex: destination,
      identityHex: normalizeDestinationHex(record.identityHex),
      destinationKind: record.destinationKind,
      announceClass: record.announceClass,
      appData: record.appData,
      displayName: record.displayName ?? existing?.displayName,
      hops: record.hops,
      interfaceHex: record.interfaceHex,
      receivedAtMs: record.receivedAtMs,
    };
  }

  function isLocalPeerDestination(destinationRaw: string): boolean {
    const destination = normalizeDestinationHex(destinationRaw);
    if (!isValidDestinationHex(destination)) {
      return false;
    }

    const localAppDestination = normalizeDestinationHex(status.value.appDestinationHex ?? "");
    const localLxmfDestination = normalizeDestinationHex(status.value.lxmfDestinationHex ?? "");
    return destination === localAppDestination || destination === localLxmfDestination;
  }

  function isLocalPeer(peer: Pick<DiscoveredPeer, "destination" | "identityHex">): boolean {
    if (isLocalPeerDestination(peer.destination)) {
      return true;
    }

    const localIdentity = normalizeDestinationHex(status.value.identityHex ?? "");
    const peerIdentity = normalizeDestinationHex(peer.identityHex ?? "");
    return isValidDestinationHex(localIdentity) && peerIdentity === localIdentity;
  }

  function isLocalDestinationIdentityPair(
    destinationRaw: string,
    identityRaw?: string,
  ): boolean {
    if (isLocalPeerDestination(destinationRaw)) {
      return true;
    }
    const localIdentity = normalizeDestinationHex(status.value.identityHex ?? "");
    const peerIdentity = normalizeDestinationHex(identityRaw ?? "");
    return isValidDestinationHex(localIdentity) && peerIdentity === localIdentity;
  }

  function resolvePeerLxmfDestinationByIdentity(identityRaw?: string): string | undefined {
    const identityHex = normalizeDestinationHex(identityRaw ?? "");
    if (!isValidDestinationHex(identityHex) || identityHex === normalizeDestinationHex(status.value.identityHex ?? "")) {
      return undefined;
    }

    const mapped = normalizeDestinationHex(lxmfDestinationByIdentity[identityHex] ?? "");
    if (isValidDestinationHex(mapped)) {
      return mapped;
    }

    return Object.values(discoveredByDestination)
      .find((peer) => normalizeDestinationHex(peer.identityHex ?? "") === identityHex)
      ?.lxmfDestinationHex;
  }

  function setPeerState(
    destinationRaw: string,
    stateValue: PeerConnectionState,
    lastErrorValue?: string,
  ): void {
    const destination = normalizeDestinationHex(destinationRaw);
    if (!isValidDestinationHex(destination)) {
      return;
    }

    upsertDiscovered(destination, {
      state: stateValue,
      lastError: lastErrorValue,
    });
  }

  function clearHubDirectoryState(): void {
    hubDirectorySnapshot.value = null;
    lastHubRefreshAt.value = 0;
  }

  function clearAnnounceState(): void {
    for (const destination of Object.keys(announceByDestination)) {
      delete announceByDestination[destination];
    }
  }

  function upsertResolvedPeer(peer: PeerRecord): void {
    const destination = normalizeDestinationHex(peer.destinationHex);
    const identityHex = normalizeDestinationHex(peer.identityHex ?? "");
    const lxmfDestinationHex = normalizeDestinationHex(peer.lxmfDestinationHex ?? "");
    const canonicalDestination = isValidDestinationHex(lxmfDestinationHex)
      ? lxmfDestinationHex
      : destination;
    if (
      !isValidDestinationHex(canonicalDestination)
      || isLocalDestinationIdentityPair(canonicalDestination, peer.identityHex)
    ) {
      return;
    }

    if (isValidDestinationHex(identityHex) && destination !== canonicalDestination) {
      appDestinationByIdentity[identityHex] = destination;
    }
    if (isValidDestinationHex(identityHex) && isValidDestinationHex(lxmfDestinationHex)) {
      lxmfDestinationByIdentity[identityHex] = lxmfDestinationHex;
    }

    const saved = nativeSavedPeerForCanonicalDestination(
      canonicalDestination,
      identityHex,
      peer.saved,
      peer.displayName,
    );
    const hasCanonicalRemAnnounce = peer.lxmfLastSeenAtMs
      ? peerHasRemAnnounceEvidence({
        appData: peer.appData,
        latestAnnounceKind: "lxmf_delivery",
        latestAnnounceClass: "LxmfDelivery",
      })
      : false;
    upsertDiscovered(
      canonicalDestination,
      {
        identityHex: isValidDestinationHex(identityHex) ? identityHex : undefined,
        lxmfDestinationHex: isValidDestinationHex(lxmfDestinationHex) ? lxmfDestinationHex : undefined,
        announcedName: peer.displayName?.trim() || undefined,
        label: saved?.label ?? undefined,
        appData: peer.appData,
        latestAnnounceKind: peer.lxmfLastSeenAtMs ? "lxmf_delivery" : undefined,
        latestAnnounceClass: peer.lxmfLastSeenAtMs ? "LxmfDelivery" : undefined,
        announceLastSeenAt: peer.announceLastSeenAtMs,
        lxmfLastSeenAt: peer.lxmfLastSeenAtMs,
        lastSeenAt: peer.lastSeenAtMs,
        state: toUiPeerState(peer.state),
        saved: Boolean(saved) || peer.saved,
        stale: peer.stale,
        activeLink: peer.activeLink,
        lastError: peer.lastResolutionError,
        lastResolutionError: peer.lastResolutionError,
        lastResolutionAttemptAt: peer.lastResolutionAttemptAtMs,
      },
      peer.hubDerived ? "hub" : hasCanonicalRemAnnounce ? "announce" : undefined,
    );
    refreshSavedPeerProfile(canonicalDestination, "native peer route profile");
  }

  function applyPeerChanged(change: PeerChangedEvent["change"]): void {
    const destination = normalizeDestinationHex(change.destinationHex);
    const identityHex = normalizeDestinationHex(change.identityHex ?? "");
    const lxmfDestinationHex = normalizeDestinationHex(change.lxmfDestinationHex ?? "");
    const canonicalDestination = isValidDestinationHex(lxmfDestinationHex)
      ? lxmfDestinationHex
      : destination;
    if (
      !isValidDestinationHex(canonicalDestination)
      || isLocalDestinationIdentityPair(canonicalDestination, change.identityHex)
    ) {
      return;
    }

    if (isValidDestinationHex(identityHex) && destination !== canonicalDestination) {
      appDestinationByIdentity[identityHex] = destination;
    }
    if (isValidDestinationHex(identityHex) && isValidDestinationHex(lxmfDestinationHex)) {
      lxmfDestinationByIdentity[identityHex] = lxmfDestinationHex;
    }

    const saved = nativeSavedPeerForCanonicalDestination(
      canonicalDestination,
      identityHex,
      change.saved,
      change.displayName,
    );
    const hasCanonicalRemAnnounce = change.lxmfLastSeenAtMs
      ? peerHasRemAnnounceEvidence({
        appData: change.appData ?? discoveredByDestination[canonicalDestination]?.appData,
        latestAnnounceKind: "lxmf_delivery",
        latestAnnounceClass: "LxmfDelivery",
      })
      : false;
    upsertDiscovered(
      canonicalDestination,
      {
        identityHex: isValidDestinationHex(identityHex)
          ? identityHex
          : undefined,
        lxmfDestinationHex: isValidDestinationHex(lxmfDestinationHex) ? lxmfDestinationHex : undefined,
        announcedName: change.displayName?.trim() || undefined,
        label: saved?.label ?? discoveredByDestination[canonicalDestination]?.label,
        appData: change.appData ?? discoveredByDestination[canonicalDestination]?.appData,
        latestAnnounceKind: change.lxmfLastSeenAtMs
          ? "lxmf_delivery"
          : discoveredByDestination[canonicalDestination]?.latestAnnounceKind,
        latestAnnounceClass: change.lxmfLastSeenAtMs
          ? "LxmfDelivery"
          : discoveredByDestination[canonicalDestination]?.latestAnnounceClass,
        state: change.state ? toUiPeerState(change.state) : undefined,
        saved: Boolean(saved) || change.saved,
        stale: change.stale,
        activeLink: change.activeLink,
        lastError: change.lastError,
        lastResolutionError: change.lastResolutionError,
        lastResolutionAttemptAt: change.lastResolutionAttemptAtMs,
        lastSeenAt: change.lastSeenAtMs,
        announceLastSeenAt: change.announceLastSeenAtMs,
        lxmfLastSeenAt: change.lxmfLastSeenAtMs,
      },
      change.hubDerived ? "hub" : hasCanonicalRemAnnounce ? "announce" : undefined,
    );
    refreshSavedPeerProfile(canonicalDestination, "peer change route profile");
  }

  function reconcileNativePeerSnapshot(peers: PeerRecord[]): void {
    const nativeDestinations = new Set(
      peers
        .map((peer) => {
          const destination = normalizeDestinationHex(peer.destinationHex);
          const lxmfDestination = normalizeDestinationHex(peer.lxmfDestinationHex ?? "");
          return isValidDestinationHex(lxmfDestination) ? lxmfDestination : destination;
        })
        .filter((destination) => isValidDestinationHex(destination)),
    );

    for (const [destination, peer] of Object.entries(discoveredByDestination)) {
      if (nativeDestinations.has(destination)) {
        continue;
      }

      const retainedSources = peer.sources.filter((source) => source === "import");
      if (retainedSources.length === 0) {
        delete discoveredByDestination[destination];
        continue;
      }

      discoveredByDestination[destination] = {
        ...peer,
        sources: retainedSources,
        identityHex: undefined,
        lxmfDestinationHex: undefined,
        announceLastSeenAt: undefined,
        lxmfLastSeenAt: undefined,
        latestAnnounceKind: undefined,
        latestAnnounceClass: undefined,
        state: peer.saved ? "connecting" : "disconnected",
        stale: false,
        activeLink: false,
        lastError: undefined,
        lastResolutionError: undefined,
      };
    }
  }

  function markPeerManagedState(destinationRaw: string, managed: boolean): void {
    const destination = normalizeDestinationHex(destinationRaw);
    if (!isValidDestinationHex(destination) || isLocalPeerDestination(destination)) {
      return;
    }
    const currentlySaved = Boolean(savedByDestination[destination] || discoveredByDestination[destination]?.saved);
    upsertDiscovered(destination, {
      saved: managed ? true : currentlySaved,
      state: managed ? "connecting" : "disconnected",
      activeLink: managed ? discoveredByDestination[destination]?.activeLink : false,
      lastError: undefined,
      lastResolutionError: undefined,
    });
  }

  async function settlePeerConnectionState(
    destinationRaw: string,
    target: "connected" | "disconnected",
  ): Promise<void> {
    const destination = normalizeDestinationHex(destinationRaw);
    if (!isValidDestinationHex(destination) || !status.value.running) {
      return;
    }

    const deadline = nowMs() + 6_000;
    do {
      await refreshMessagingState();
      const peer = discoveredByDestination[destination];
      if (!peer) {
        return;
      }
      if (target === "connected" && peer.activeLink) {
        return;
      }
      if (target === "disconnected" && !peer.activeLink) {
        return;
      }
      await sleep(400);
    } while (nowMs() < deadline);
  }

  function describePeerState(destinationRaw: string): string {
    const destination = normalizeDestinationHex(destinationRaw);
    const peer = discoveredByDestination[destination];
    if (!peer) {
      return `destination=${destination} state=missing`;
    }

    return [
      `destination=${destination}`,
      `state=${peer.state}`,
      `saved=${peer.saved}`,
      `stale=${peer.stale}`,
      `activeLink=${peer.activeLink}`,
      `label=${peer.label ?? "-"}`,
      `announced=${peer.announcedName ?? "-"}`,
      `identity=${peer.identityHex ?? "-"}`,
      `lxmf=${peer.lxmfDestinationHex ?? "-"}`,
      `sources=${peer.sources.join("+") || "-"}`,
    ].join(" ");
  }

  return {
    applyPeerChanged,
    clearAnnounceState,
    clearHubDirectoryState,
    describePeerState,
    isLocalDestinationIdentityPair,
    isLocalPeer,
    isLocalPeerDestination,
    markPeerManagedState,
    reconcileNativePeerSnapshot,
    resolvePeerLxmfDestinationByIdentity,
    setPeerState,
    settlePeerConnectionState,
    upsertDiscovered,
    upsertNativeAnnounceRecord,
    upsertResolvedPeer,
  };
}
