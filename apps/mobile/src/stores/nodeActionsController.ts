import { YELLOW_TEAM_UID, type NodeStatus, type ReticulumNodeClient } from "@reticulum/node-client";
import type { Ref, ShallowRef } from "vue";

import { clearHubRegistryLinkage } from "../services/hubRegistryBootstrap";
import type { DiscoveredPeer, NodeUiSettings, PeerListV1, SavedPeer } from "../types/domain";
import { persistUiSettingsProjection as storeUiSettingsProjection } from "../utils/legacyState";
import {
  createPeerListV1,
  ensureRequiredAnnounceCapabilities,
  isValidDestinationHex,
  normalizeDestinationHex,
  parsePeerListV1,
} from "../utils/peers";
import { normalizeRnodeSettings } from "../utils/rnodeProfiles";
import { normalizeTcpCommunityClients } from "../utils/tcpCommunityServers";
import {
  hasSelectedHubIdentity,
  hubModeUsesRch,
  nodeConfigsEqual,
  normalizeAppSettingsRecord,
  normalizeHubMode,
  normalizeStoredDisplayName,
  normalizeTelemetrySettings,
  toAppSettingsRecord,
  toNodeConfig,
  toUiSettingsProjection,
} from "./nodeSettingsModel";
import type { HubRegistrationSnapshot } from "./nodeStoreCore";
import { nowMs } from "./nodeStoreCore";

interface NodeActionsContext {
  appendLog: (level: string, message: string) => void;
  captureActionError: (action: string, error: unknown) => Error;
  clearHubDirectoryState: () => void;
  clearLastError: () => void;
  clearPeerRemoved: (destination: string, peer?: DiscoveredPeer) => void;
  client: ShallowRef<ReticulumNodeClient | null>;
  defaultsWithTcpFallback: () => string[];
  discoveredByDestination: Record<string, DiscoveredPeer>;
  errorMessage: (error: unknown) => string;
  hubRegistration: HubRegistrationSnapshot;
  init: () => Promise<void>;
  lastError: Ref<string>;
  markPeerRemoved: (destination: string, peer?: DiscoveredPeer) => string[];
  peerByAnyKnownDestination: (
    peers: Record<string, DiscoveredPeer>,
    destination: string,
  ) => DiscoveredPeer | undefined;
  persistSavedPeersProjection: (
    peers: Record<string, SavedPeer>,
    reason?: string,
  ) => Promise<void>;
  persistSettingsProjection: (settings: NodeUiSettings) => Promise<void>;
  refreshHubRegistrationState: (attemptBootstrap?: boolean) => Promise<void>;
  savedByDestination: Record<string, SavedPeer>;
  savedPeerProfileFromDiscovered: (
    destination: string,
    discovered?: DiscoveredPeer,
    fallback?: Partial<SavedPeer>,
  ) => SavedPeer;
  setHubRegistrationPending: (message?: string) => void;
  setNodeConfigRestartRequired: (required: boolean) => void;
  settings: NodeUiSettings;
  status: Ref<NodeStatus>;
  upsertDiscovered: (
    destination: string,
    patch: Partial<DiscoveredPeer>,
    source?: "announce" | "hub" | "import",
  ) => void;
}

export function createNodeActionsController(context: NodeActionsContext) {
  const {
    appendLog,
    captureActionError,
    clearHubDirectoryState,
    clearLastError,
    clearPeerRemoved,
    client,
    defaultsWithTcpFallback,
    discoveredByDestination,
    errorMessage,
    hubRegistration,
    init,
    lastError,
    markPeerRemoved,
    peerByAnyKnownDestination,
    persistSavedPeersProjection,
    persistSettingsProjection,
    refreshHubRegistrationState,
    savedByDestination,
    savedPeerProfileFromDiscovered,
    setHubRegistrationPending,
    setNodeConfigRestartRequired,
    settings,
    status,
    upsertDiscovered,
  } = context;

  function refreshHubRegistrationDetached(attemptBootstrap: boolean): void {
    void refreshHubRegistrationState(attemptBootstrap).catch((error: unknown) => {
      appendLog("Warn", `Hub registration refresh failed: ${errorMessage(error)}`);
    });
  }

  async function refreshHubDirectory(): Promise<void> {
    try {
      if (!hubModeUsesRch(settings.hub.mode)) {
        clearHubDirectoryState();
        return;
      }
      if (!hasSelectedHubIdentity(settings.hub.identityHash)) {
        clearHubDirectoryState();
        if (settings.hub.mode === "Connected") {
          throw new Error("Connected mode requires selecting an RCH hub before refreshing.");
        }
        return;
      }
      if (!client.value || !status.value.running) {
        return;
      }
      clearLastError();
      await client.value.refreshHubDirectory();
    } catch (error: unknown) {
      throw captureActionError("Hub directory refresh failed", error);
    }
  }

  async function forgetHubRegistryLinkage(): Promise<void> {
    clearHubRegistryLinkage();
    hubRegistration.linkage = undefined;
    hubRegistration.lastReadyAt = undefined;
    setHubRegistrationPending("Hub registry linkage cleared.");
  }

  async function setAnnounceCapabilities(capabilityString: string): Promise<void> {
    settings.announceCapabilities = ensureRequiredAnnounceCapabilities(capabilityString);
    const nextSettings = normalizeAppSettingsRecord(
      toAppSettingsRecord(settings),
      toUiSettingsProjection(settings),
      defaultsWithTcpFallback(),
      true,
    );
    await init();
    await persistSettingsProjection(nextSettings);

    if (!client.value || !status.value.running) {
      return;
    }
    try {
      clearLastError();
      await client.value.setAnnounceCapabilities(
        ensureRequiredAnnounceCapabilities(settings.announceCapabilities),
      );
    } catch (error: unknown) {
      throw captureActionError("Set announce capabilities failed", error);
    }
  }

  async function savePeer(destinationRaw: string): Promise<void> {
    await init();
    const requestedDestination = normalizeDestinationHex(destinationRaw);
    if (!isValidDestinationHex(requestedDestination)) {
      return;
    }
    const discovered = peerByAnyKnownDestination(discoveredByDestination, requestedDestination);
    const destination = normalizeDestinationHex(
      discovered?.lxmfDestinationHex ?? discovered?.destination ?? requestedDestination,
    );
    if (!isValidDestinationHex(destination)) {
      return;
    }
    clearPeerRemoved(requestedDestination, discovered);
    clearPeerRemoved(destination, discovered);
    const nextSavedPeers = {
      ...savedByDestination,
      [destination]: savedPeerProfileFromDiscovered(destination, discovered, {
        label: discovered?.label,
        savedAt: nowMs(),
      }),
    };
    if (requestedDestination !== destination) {
      delete nextSavedPeers[requestedDestination];
    }
    await persistSavedPeersProjection(nextSavedPeers, `explicit save ${destination}`);
    const localTeams = settings.teams.localTeamsInitialized
      ? settings.teams.localTeams.map((team) => ({
          ...team,
          memberDestinations: team.teamUid === YELLOW_TEAM_UID
            ? [...new Set([...team.memberDestinations, destination])]
            : [...team.memberDestinations],
        }))
      : [{
          teamUid: YELLOW_TEAM_UID,
          memberDestinations: Object.keys(nextSavedPeers),
        }];
    if (!localTeams.some((team) => team.teamUid === YELLOW_TEAM_UID)) {
      localTeams.unshift({ teamUid: YELLOW_TEAM_UID, memberDestinations: [destination] });
    }
    await updateSettings({
      teams: {
        ...settings.teams,
        localTeams,
        localTeamsInitialized: true,
      },
    });
  }

  async function removePeer(destinationRaw: string): Promise<void> {
    await init();
    const destination = normalizeDestinationHex(destinationRaw);
    if (!isValidDestinationHex(destination)) {
      return;
    }
    const discovered = peerByAnyKnownDestination(discoveredByDestination, destination);
    const removedDestinations = markPeerRemoved(destination, discovered);
    const nextSavedPeers = { ...savedByDestination };
    for (const removedDestination of removedDestinations) {
      delete nextSavedPeers[removedDestination];
      delete discoveredByDestination[removedDestination];
    }
    await persistSavedPeersProjection(nextSavedPeers, `explicit remove ${destination}`);
    const retainedDestinations = new Set(Object.keys(nextSavedPeers));
    await updateSettings({
      teams: {
        ...settings.teams,
        localTeams: settings.teams.localTeams.map((team) => ({
          ...team,
          memberDestinations: team.memberDestinations.filter((member) => (
            retainedDestinations.has(member)
          )),
        })),
        localTeamsInitialized: true,
      },
    });
    if (client.value && status.value.running) {
      try {
        await client.value.disconnectPeer(destination);
      } catch (error: unknown) {
        appendLog("Debug", `[peers] remove disconnect skipped destination=${destination}: ${errorMessage(error)}`);
      }
    }
  }

  async function unsavePeer(destinationRaw: string): Promise<void> {
    await init();
    const destination = normalizeDestinationHex(destinationRaw);
    const nextSavedPeers = { ...savedByDestination };
    delete nextSavedPeers[destination];
    const discovered = peerByAnyKnownDestination(discoveredByDestination, destination);
    const canonicalDestination = normalizeDestinationHex(
      discovered?.lxmfDestinationHex ?? discovered?.destination ?? "",
    );
    if (isValidDestinationHex(canonicalDestination)) {
      delete nextSavedPeers[canonicalDestination];
    }
    await persistSavedPeersProjection(nextSavedPeers, `explicit unsave ${destination}`);
    const retainedDestinations = new Set(Object.keys(nextSavedPeers));
    await updateSettings({
      teams: {
        ...settings.teams,
        localTeams: settings.teams.localTeams.map((team) => ({
          ...team,
          memberDestinations: team.memberDestinations.filter((member) => (
            retainedDestinations.has(member)
          )),
        })),
        localTeamsInitialized: true,
      },
    });
  }

  async function setPeerLabel(destinationRaw: string, label: string): Promise<void> {
    await init();
    const destination = normalizeDestinationHex(destinationRaw);
    const normalizedLabel = label.trim();
    if (savedByDestination[destination]) {
      const nextSavedPeers = {
        ...savedByDestination,
        [destination]: {
          ...savedByDestination[destination],
          label: normalizedLabel || undefined,
        },
      };
      await persistSavedPeersProjection(nextSavedPeers, `label update ${destination}`);
    }
    if (discoveredByDestination[destination]) {
      discoveredByDestination[destination].label = normalizedLabel || undefined;
    }
  }

  async function updateSettings(next: Partial<NodeUiSettings>): Promise<void> {
    let uiSettingsChanged = false;
    let hubRoutingChanged = false;
    const previousNodeConfig = toNodeConfig(settings);
    if (next.displayName !== undefined) {
      settings.displayName = normalizeStoredDisplayName(next.displayName);
    }
    if (next.clientMode) {
      settings.clientMode = next.clientMode;
      uiSettingsChanged = true;
    }
    settings.autoConnectSaved = false;
    if (next.announceCapabilities !== undefined) {
      settings.announceCapabilities = ensureRequiredAnnounceCapabilities(next.announceCapabilities);
    }
    if (next.tcpClients !== undefined) {
      settings.tcpClients = normalizeTcpCommunityClients(next.tcpClients, defaultsWithTcpFallback(), true);
    }
    if (typeof next.broadcast === "boolean") {
      settings.broadcast = next.broadcast;
    }
    if (typeof next.transportNodeEnabled === "boolean") {
      settings.transportNodeEnabled = next.transportNodeEnabled;
    }
    if (next.announceIntervalSeconds !== undefined) {
      settings.announceIntervalSeconds = next.announceIntervalSeconds;
    }
    if (next.telemetry) {
      settings.telemetry = normalizeTelemetrySettings(next.telemetry, settings.telemetry);
    }
    if (next.hub) {
      const previousHubMode = settings.hub.mode;
      const previousHubIdentityHash = settings.hub.identityHash;
      settings.hub = {
        ...settings.hub,
        ...next.hub,
        mode: normalizeHubMode(next.hub.mode ?? settings.hub.mode),
      };
      hubRoutingChanged = settings.hub.mode !== previousHubMode
        || settings.hub.identityHash !== previousHubIdentityHash;
      if (
        !hubModeUsesRch(settings.hub.mode)
        || settings.hub.mode !== previousHubMode
        || settings.hub.identityHash !== previousHubIdentityHash
      ) {
        clearHubDirectoryState();
      }
    }
    if (next.teams) {
      settings.teams = {
        ...settings.teams,
        ...next.teams,
        aliases: next.teams.aliases?.map((alias) => ({ ...alias }))
          ?? settings.teams.aliases.map((alias) => ({ ...alias })),
        localTeams: next.teams.localTeams?.map((team) => ({
          ...team,
          memberDestinations: [...team.memberDestinations],
        })) ?? settings.teams.localTeams.map((team) => ({
          ...team,
          memberDestinations: [...team.memberDestinations],
        })),
      };
    }
    if (next.rnode) {
      settings.rnode = normalizeRnodeSettings({ ...settings.rnode, ...next.rnode });
    }
    const nextSettings = normalizeAppSettingsRecord(
      toAppSettingsRecord(settings),
      toUiSettingsProjection(settings),
      defaultsWithTcpFallback(),
      true,
    );
    if (uiSettingsChanged) {
      storeUiSettingsProjection(toUiSettingsProjection(settings));
    }
    await init();
    try {
      await persistSettingsProjection(nextSettings);
    } catch (error: unknown) {
      appendLog("Warn", `Settings projection persist failed: ${errorMessage(error)}`);
      throw error;
    }
    const nodeConfigChanged = !nodeConfigsEqual(previousNodeConfig, toNodeConfig(settings));
    if (status.value.running && nodeConfigChanged) {
      setNodeConfigRestartRequired(true);
      appendLog("Info", "Node interface settings changed. Restart the app or node to apply them.");
    }
    if (!hubRoutingChanged || !status.value.running || !hubModeUsesRch(settings.hub.mode)) {
      refreshHubRegistrationDetached(hubModeUsesRch(settings.hub.mode));
      return;
    }
    if (!hasSelectedHubIdentity(settings.hub.identityHash)) {
      if (settings.hub.mode === "Connected") {
        const message = "Connected mode requires selecting an RCH hub before outbound traffic can be routed.";
        lastError.value = message;
        appendLog("Warn", message);
      }
      refreshHubRegistrationDetached(hubModeUsesRch(settings.hub.mode));
      return;
    }
    appendLog(
      "Info",
      "Hub routing settings changed. Restart the node to apply the selected hub and refresh from the hub directory.",
    );
    refreshHubRegistrationDetached(hubModeUsesRch(settings.hub.mode));
  }

  function getSavedPeerList(): PeerListV1 {
    return createPeerListV1(Object.values(savedByDestination));
  }

  function importPeerList(peerList: PeerListV1, mode: "merge" | "replace" = "merge"): void {
    if (mode === "replace") {
      for (const key of Object.keys(savedByDestination)) {
        delete savedByDestination[key];
      }
    }
    for (const peer of peerList.peers) {
      const destination = normalizeDestinationHex(peer.destination);
      if (!isValidDestinationHex(destination)) {
        continue;
      }
      savedByDestination[destination] = {
        destination,
        label: peer.label?.trim() || undefined,
        savedAt: nowMs(),
        lxmfDestinationHex: destination,
      };
      upsertDiscovered(destination, {
        label: peer.label?.trim() || undefined,
        saved: true,
        lastSeenAt: discoveredByDestination[destination]?.lastSeenAt ?? 0,
      }, "import");
    }
    void init()
      .then(() => persistSavedPeersProjection({ ...savedByDestination }, `peer list import (${mode})`))
      .catch((error: unknown) => {
        appendLog("Warn", `Saved-peer projection persist failed: ${errorMessage(error)}`);
      });
  }

  function parsePeerListText(text: string): ReturnType<typeof parsePeerListV1> {
    return parsePeerListV1(text);
  }

  return {
    forgetHubRegistryLinkage,
    getSavedPeerList,
    importPeerList,
    parsePeerListText,
    refreshHubDirectory,
    removePeer,
    savePeer,
    setAnnounceCapabilities,
    setPeerLabel,
    unsavePeer,
    updateSettings,
  };
}
