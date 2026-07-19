import {
  type AnnounceReceivedEvent,
  type AnnounceRecord,
  type NodeStatus,
  type ReticulumNodeClient,
} from "@reticulum/node-client";
import type { Ref, ShallowRef } from "vue";

import type {
  DiscoveredPeer,
  SavedPeer,
} from "../types/domain";
import { peerHasRemAnnounceEvidence } from "../utils/announceEvidence";
import {
  extractAnnouncedName,
  hasCapability,
  isValidDestinationHex,
  normalizeDestinationHex,
} from "../utils/peers";
import {
  PEER_ONLINE_FRESHNESS_MS,
  STARTUP_ANNOUNCE_SETTLE_MS,
  advancePresenceNow,
  nowMs,
  sleep,
} from "./nodeStoreCore";
import { runDetachedStoreTask } from "../utils/detachedStoreTask";

interface NodeAnnounceContext {
  appendLog: (level: string, message: string) => void;
  appDestinationByIdentity: Record<string, string>;
  client: ShallowRef<ReticulumNodeClient | null>;
  discoveredByDestination: Record<string, DiscoveredPeer>;
  liveLxmfPresenceByIdentity: Record<string, number>;
  livePresenceByDestination: Record<string, number>;
  lxmfDestinationByIdentity: Record<string, string>;
  errorMessage: (error: unknown) => string;
  isLocalDestinationIdentityPair: (destination: string, identity: string) => boolean;
  peerByAnyKnownDestination: (
    peers: Record<string, DiscoveredPeer>,
    destination: string,
  ) => DiscoveredPeer | undefined;
  persistSavedPeersProjection: (
    nextSavedPeers: Record<string, SavedPeer>,
    reason?: string,
  ) => Promise<void>;
  presenceNow: Ref<number>;
  savedByDestination: Record<string, SavedPeer>;
  setLastError: (message: string) => void;
  startupSettling: Ref<boolean>;
  status: Ref<NodeStatus>;
  refreshMessagingState: () => Promise<void>;
  upsertDiscovered: (
    destination: string,
    patch: Partial<DiscoveredPeer>,
    source?: "announce" | "hub" | "import",
  ) => void;
  upsertNativeAnnounceRecord: (record: AnnounceRecord) => void;
}

export function createNodeAnnounceController(context: NodeAnnounceContext) {
  const {
    appendLog,
    appDestinationByIdentity,
    client,
    discoveredByDestination,
    liveLxmfPresenceByIdentity,
    livePresenceByDestination,
    lxmfDestinationByIdentity,
    errorMessage,
    isLocalDestinationIdentityPair,
    peerByAnyKnownDestination,
    persistSavedPeersProjection,
    presenceNow,
    savedByDestination,
    setLastError,
    startupSettling,
    status,
    refreshMessagingState,
    upsertDiscovered,
    upsertNativeAnnounceRecord,
  } = context;

  function persistSavedPeersDetached(reason: string): void {
    runDetachedStoreTask(
      { setLastError, logUi: appendLog },
      "saved-peers",
      reason,
      () => persistSavedPeersProjection({ ...savedByDestination }, reason),
    );
  }

  function recordLivePresence(
    destinationKind: "app" | "lxmf_delivery" | "lxmf_propagation" | "other",
    destinationHex: string,
    identityHex: string | undefined,
    receivedAtMs: number,
  ): void {
    if (destinationKind === "lxmf_propagation") {
      return;
    }

    if (destinationKind === "lxmf_delivery") {
      if (isValidDestinationHex(destinationHex)) {
        livePresenceByDestination[destinationHex] = Math.max(
          livePresenceByDestination[destinationHex] ?? 0,
          receivedAtMs,
        );
      }
      if (isValidDestinationHex(identityHex ?? "")) {
        const normalizedIdentity = normalizeDestinationHex(identityHex ?? "");
        liveLxmfPresenceByIdentity[normalizedIdentity] = Math.max(
          liveLxmfPresenceByIdentity[normalizedIdentity] ?? 0,
          receivedAtMs,
        );
        const appDestinationHex = appDestinationByIdentity[normalizedIdentity];
        if (isValidDestinationHex(appDestinationHex)) {
          livePresenceByDestination[appDestinationHex] = Math.max(
            livePresenceByDestination[appDestinationHex] ?? 0,
            receivedAtMs,
          );
        }
      }
      return;
    }

    if (!isValidDestinationHex(destinationHex)) {
      return;
    }
    livePresenceByDestination[destinationHex] = Math.max(
      livePresenceByDestination[destinationHex] ?? 0,
      receivedAtMs,
    );
    if (isValidDestinationHex(identityHex ?? "")) {
      const normalizedIdentity = normalizeDestinationHex(identityHex ?? "");
      const lxmfSeenAt = liveLxmfPresenceByIdentity[normalizedIdentity];
      if (typeof lxmfSeenAt === "number") {
        livePresenceByDestination[destinationHex] = Math.max(
          livePresenceByDestination[destinationHex],
          lxmfSeenAt,
        );
      }
    }
  }

  function migrateSavedPeerAlias(
    aliasDestinationRaw: string | undefined,
    canonicalDestinationRaw: string,
  ): SavedPeer | undefined {
    const aliasDestination = normalizeDestinationHex(aliasDestinationRaw ?? "");
    const canonicalDestination = normalizeDestinationHex(canonicalDestinationRaw);
    if (
      !isValidDestinationHex(aliasDestination)
      || !isValidDestinationHex(canonicalDestination)
      || aliasDestination === canonicalDestination
    ) {
      return savedByDestination[canonicalDestination];
    }

    const aliasPeer = savedByDestination[aliasDestination];
    if (!aliasPeer) {
      return savedByDestination[canonicalDestination];
    }

    const existingPeer = savedByDestination[canonicalDestination];
    const migratedPeer: SavedPeer = {
      ...aliasPeer,
      ...existingPeer,
      destination: canonicalDestination,
      label: existingPeer?.label ?? aliasPeer.label,
      savedAt: existingPeer?.savedAt ?? aliasPeer.savedAt,
      identityHex: existingPeer?.identityHex ?? aliasPeer.identityHex,
      lxmfDestinationHex: existingPeer?.lxmfDestinationHex ?? aliasPeer.lxmfDestinationHex,
      appData: existingPeer?.appData ?? aliasPeer.appData,
      displayName: existingPeer?.displayName ?? aliasPeer.displayName,
      lastRouteSeenAtMs: existingPeer?.lastRouteSeenAtMs ?? aliasPeer.lastRouteSeenAtMs,
      lastHops: existingPeer?.lastHops ?? aliasPeer.lastHops,
    };
    delete savedByDestination[aliasDestination];
    savedByDestination[canonicalDestination] = migratedPeer;
    if (discoveredByDestination[aliasDestination]) {
      delete discoveredByDestination[aliasDestination];
    }
    persistSavedPeersDetached(`canonical saved peer ${canonicalDestination}`);
    return migratedPeer;
  }

  function savedPeerProfileFromDiscovered(
    destinationRaw: string,
    discovered?: DiscoveredPeer,
    fallback?: Partial<SavedPeer>,
  ): SavedPeer {
    const destination = normalizeDestinationHex(destinationRaw);
    const identityHex = normalizeDestinationHex(discovered?.identityHex ?? fallback?.identityHex ?? "");
    const lxmfDestinationHex = normalizeDestinationHex(
      discovered?.lxmfDestinationHex ?? fallback?.lxmfDestinationHex ?? "",
    );
    const routeSeenAt = Math.max(
      discovered?.lxmfLastSeenAt ?? 0,
      discovered?.announceLastSeenAt ?? 0,
      discovered?.lastSeenAt ?? 0,
      fallback?.lastRouteSeenAtMs ?? 0,
    );
    const hops = typeof discovered?.hops === "number" && Number.isFinite(discovered.hops)
      ? discovered.hops
      : fallback?.lastHops;

    return {
      destination,
      label: discovered?.label ?? fallback?.label,
      savedAt: Number(fallback?.savedAt ?? nowMs()),
      identityHex: isValidDestinationHex(identityHex) ? identityHex : undefined,
      lxmfDestinationHex: isValidDestinationHex(lxmfDestinationHex) ? lxmfDestinationHex : undefined,
      appData: discovered?.appData?.trim() || fallback?.appData?.trim() || undefined,
      displayName: discovered?.announcedName?.trim() || fallback?.displayName?.trim() || undefined,
      lastRouteSeenAtMs: routeSeenAt > 0 ? routeSeenAt : undefined,
      lastHops: typeof hops === "number" && Number.isFinite(hops) ? hops : undefined,
    };
  }

  function sameSavedPeerProfile(left: SavedPeer, right: SavedPeer): boolean {
    return left.destination === right.destination
      && left.label === right.label
      && left.savedAt === right.savedAt
      && left.identityHex === right.identityHex
      && left.lxmfDestinationHex === right.lxmfDestinationHex
      && left.appData === right.appData
      && left.displayName === right.displayName
      && left.lastRouteSeenAtMs === right.lastRouteSeenAtMs
      && left.lastHops === right.lastHops;
  }

  function refreshSavedPeerProfile(destinationRaw: string, reason: string): void {
    const destination = normalizeDestinationHex(destinationRaw);
    const saved = savedByDestination[destination];
    const discovered = discoveredByDestination[destination];
    if (!saved || !discovered) {
      return;
    }

    const next = savedPeerProfileFromDiscovered(destination, discovered, saved);
    if (sameSavedPeerProfile(saved, next)) {
      return;
    }
    savedByDestination[destination] = next;
    persistSavedPeersDetached(`${reason} ${destination}`);
  }

  function nativeSavedPeerForCanonicalDestination(
    canonicalDestinationRaw: string,
    identityHexRaw: string | undefined,
    nativeSaved: boolean,
    displayName?: string,
  ): SavedPeer | undefined {
    const canonicalDestination = normalizeDestinationHex(canonicalDestinationRaw);
    if (!isValidDestinationHex(canonicalDestination)) {
      return undefined;
    }

    const identityHex = normalizeDestinationHex(identityHexRaw ?? "");
    const aliasDestination = isValidDestinationHex(identityHex)
      ? appDestinationByIdentity[identityHex]
      : undefined;
    const saved = migrateSavedPeerAlias(aliasDestination, canonicalDestination)
      ?? savedByDestination[canonicalDestination];
    if (saved || !nativeSaved) {
      return saved;
    }

    const existing = peerByAnyKnownDestination(discoveredByDestination, canonicalDestination);
    const adoptedPeer = savedPeerProfileFromDiscovered(canonicalDestination, existing, {
      label: displayName?.trim() || undefined,
      displayName: displayName?.trim() || undefined,
      savedAt: nowMs(),
    });
    savedByDestination[canonicalDestination] = adoptedPeer;
    persistSavedPeersDetached(`native saved peer ${canonicalDestination}`);
    return adoptedPeer;
  }

  function applyAnnounceUpdate(
    event: AnnounceReceivedEvent | AnnounceRecord,
    source: "live" | "snapshot" = "live",
  ): void {
    const identityHex = normalizeDestinationHex(event.identityHex ?? "");
    if (isLocalDestinationIdentityPair(event.destinationHex, identityHex)) {
      return;
    }
    if (source === "live") {
      recordLivePresence(
        event.destinationKind,
        normalizeDestinationHex(event.destinationHex),
        identityHex,
        event.receivedAtMs,
      );
    }
    if (event.destinationKind === "lxmf_propagation") {
      return;
    }
    if (event.destinationKind === "lxmf_delivery") {
      const destination = normalizeDestinationHex(event.destinationHex);
      const announcedName = ("displayName" in event && typeof event.displayName === "string"
        ? event.displayName.trim()
        : undefined) ?? extractAnnouncedName(event.appData);
      if (isValidDestinationHex(identityHex)) {
        lxmfDestinationByIdentity[identityHex] = destination;
      }
      if (!peerHasRemAnnounceEvidence({
        appData: event.appData,
        latestAnnounceKind: event.destinationKind,
        latestAnnounceClass: event.announceClass,
      })) {
        return;
      }
      presenceNow.value = advancePresenceNow(presenceNow.value, event.receivedAtMs);
      const aliasDestination = isValidDestinationHex(identityHex)
        ? appDestinationByIdentity[identityHex]
        : undefined;
      const saved = migrateSavedPeerAlias(aliasDestination, destination)
        ?? savedByDestination[destination];
      upsertDiscovered(destination, {
        identityHex: isValidDestinationHex(identityHex) ? identityHex : undefined,
        lxmfDestinationHex: destination,
        lxmfLastSeenAt: event.receivedAtMs,
        announceLastSeenAt: event.receivedAtMs,
        lastSeenAt: event.receivedAtMs,
        announcedName,
        appData: event.appData,
        hops: event.hops,
        interfaceHex: event.interfaceHex,
        latestAnnounceKind: event.destinationKind,
        latestAnnounceClass: event.announceClass,
        label: saved?.label,
        saved: Boolean(saved),
      }, "announce");
      return;
    }

    if (isValidDestinationHex(identityHex)) {
      appDestinationByIdentity[identityHex] = event.destinationHex;
    }
  }

  async function refreshAnnounceState(): Promise<void> {
    if (!client.value || !status.value.running) {
      return;
    }
    try {
      const announces = await client.value.listAnnounces();
      for (const announce of announces) {
        upsertNativeAnnounceRecord(announce);
        applyAnnounceUpdate(announce, "snapshot");
      }
    } catch (error: unknown) {
      appendLog("Debug", `Announce snapshot refresh skipped: ${errorMessage(error)}`);
    }
  }

  async function settleStartupDiscovery(): Promise<void> {
    if (!status.value.running) {
      return;
    }
    startupSettling.value = true;
    try {
      await sleep(STARTUP_ANNOUNCE_SETTLE_MS);
      await refreshMessagingState();
      await refreshMessagingState();
    } finally {
      startupSettling.value = false;
    }
  }

  return {
    applyAnnounceUpdate,
    nativeSavedPeerForCanonicalDestination,
    recordLivePresence,
    refreshAnnounceState,
    refreshSavedPeerProfile,
    savedPeerProfileFromDiscovered,
    settleStartupDiscovery,
  };
}
