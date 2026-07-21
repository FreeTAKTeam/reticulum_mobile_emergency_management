import {
  type NodeStatus,
  type ReticulumNodeClient,
  type SyncStatus,
} from "@reticulum/node-client";
import type { Ref, ShallowRef } from "vue";

import type {
  DiscoveredPeer,
  HubDirectorySnapshot,
  NodeUiSettings,
  SavedPeer,
} from "../types/domain";
import { isValidDestinationHex, normalizeDestinationHex } from "../utils/peers";
import { hubModeUsesRch, toNodeConfig } from "./nodeSettingsModel";
import {
  EMPTY_SYNC_STATUS,
  NODE_START_TIMEOUT_MS,
  PEER_PRESENCE_TICK_MS,
  nowMs,
  withTimeout,
} from "./nodeStoreCore";

interface NodeLifecycleContext {
  appendLog: (level: string, message: string) => void;
  appendNodeControlEntry: (level: string, message: string, at?: number) => void;
  applyRnodeInterfaceReadiness: (at?: number) => void;
  bindClientEvents: (client: ReticulumNodeClient) => void;
  buildClient: () => ReticulumNodeClient;
  captureActionError: (action: string, error: unknown) => Error;
  captureRuntimeActionError: (action: string, error: unknown) => Error;
  clearAnnounceState: () => void;
  clearLastError: () => void;
  clearPeerRemoved: (destination: string, peer?: DiscoveredPeer) => void;
  clearReadinessError: () => void;
  client: ShallowRef<ReticulumNodeClient | null>;
  configureClientLogging: () => Promise<void>;
  describePeerState: (destination: string) => string;
  discoveredByDestination: Record<string, DiscoveredPeer>;
  errorMessage: (error: unknown) => string;
  importLegacyProjectionState: () => Promise<void>;
  hubDirectorySnapshot: Ref<HubDirectorySnapshot | null>;
  initialized: Ref<boolean>;
  isLocalPeerDestination: (destination: string) => boolean;
  logUi: (level: string, message: string) => void;
  markPeerManagedState: (destination: string, managed: boolean) => void;
  presenceNow: Ref<number>;
  peerByAnyKnownDestination: (
    peers: Record<string, DiscoveredPeer>,
    destination: string,
  ) => DiscoveredPeer | undefined;
  refreshAnnounceState: () => Promise<void>;
  refreshHubRegistrationState: (attemptBootstrap?: boolean) => Promise<void>;
  refreshMessagingState: () => Promise<void>;
  refreshOperationalSummaryProjection: () => Promise<void>;
  refreshPluginProjection: (discover?: boolean) => Promise<void>;
  refreshPluginSensors: () => Promise<void>;
  refreshSavedPeersProjection: () => Promise<void>;
  refreshSettingsProjection: () => Promise<void>;
  refreshStatusSnapshot: (retries?: number, delayMs?: number) => Promise<NodeStatus>;
  savedByDestination: Record<string, SavedPeer>;
  setLastError: (message: string) => void;
  setNodeConfigRestartRequired: (required: boolean) => void;
  setPeerState: (destination: string, state: import("../types/domain").PeerConnectionState, error?: string) => void;
  settings: NodeUiSettings;
  settlePeerConnectionState: (
    destination: string,
    target: "connected" | "disconnected",
    timeoutMs?: number,
  ) => Promise<void>;
  settleStartupDiscovery: () => Promise<void>;
  status: Ref<NodeStatus>;
  syncRuntimeSnapshot: (reason: string) => Promise<void>;
  syncStatus: Ref<SyncStatus>;
}

export function createNodeLifecycleController(context: NodeLifecycleContext) {
  const {
    appendLog,
    appendNodeControlEntry,
    applyRnodeInterfaceReadiness,
    bindClientEvents,
    buildClient,
    captureActionError,
    captureRuntimeActionError,
    clearAnnounceState,
    clearLastError,
    clearPeerRemoved,
    clearReadinessError,
    client,
    configureClientLogging,
    describePeerState,
    discoveredByDestination,
    errorMessage,
    importLegacyProjectionState,
    hubDirectorySnapshot,
    initialized,
    isLocalPeerDestination,
    logUi,
    markPeerManagedState,
    presenceNow,
    peerByAnyKnownDestination,
    refreshAnnounceState,
    refreshHubRegistrationState,
    refreshMessagingState,
    refreshOperationalSummaryProjection,
    refreshPluginProjection,
    refreshPluginSensors,
    refreshSavedPeersProjection,
    refreshSettingsProjection,
    refreshStatusSnapshot,
    savedByDestination,
    setLastError,
    setNodeConfigRestartRequired,
    setPeerState,
    settings,
    settlePeerConnectionState,
    settleStartupDiscovery,
    status,
    syncRuntimeSnapshot,
    syncStatus,
  } = context;
  let presenceTickerId: number | null = null;
  let initPromise: Promise<void> | null = null;

  async function init(): Promise<void> {
    if (initPromise) {
      return initPromise;
    }
    if (initialized.value) {
      return;
    }

    initPromise = (async () => {
      client.value = buildClient();
      bindClientEvents(client.value);
      await importLegacyProjectionState();
      await Promise.all([
        refreshSettingsProjection(),
        refreshSavedPeersProjection(),
        refreshOperationalSummaryProjection(),
        refreshPluginProjection(true),
        refreshPluginSensors(),
      ]);
      await syncRuntimeSnapshot("client init");
      if (presenceTickerId === null) {
        presenceTickerId = window.setInterval(() => {
          presenceNow.value = nowMs();
          void refreshPluginSensors();
        }, PEER_PRESENCE_TICK_MS);
      }
      await refreshHubRegistrationState(false);
      initialized.value = true;
    })()
      .finally(() => {
        initPromise = null;
      });

    return initPromise;
  }

  async function startNode(): Promise<void> {
    try {
      await init();
      if (!client.value) {
        return;
      }

      clearLastError();
      clearReadinessError();
      await withTimeout(
        client.value.start(toNodeConfig(settings)),
        NODE_START_TIMEOUT_MS,
        `node runtime start timed out after ${NODE_START_TIMEOUT_MS}ms`,
      );
      setNodeConfigRestartRequired(false);
      await refreshStatusSnapshot(8, 250);
      applyRnodeInterfaceReadiness();
      await refreshMessagingState();
      await refreshAnnounceState();
      await refreshOperationalSummaryProjection();
      await configureClientLogging();
      await settleStartupDiscovery();
      void refreshHubRegistrationState(true).catch((error: unknown) => {
        appendNodeControlEntry(
          "Warn",
          `Hub registration bootstrap failed after start: ${errorMessage(error)}`,
        );
      });
      appendNodeControlEntry("Info", "Node started.");

    } catch (error: unknown) {
      throw captureRuntimeActionError("Start node failed", error);
    }
  }

  async function stopNode(): Promise<void> {
    try {
      if (!client.value) {
        return;
      }
      clearLastError();
      clearReadinessError();
      await client.value.stop();
      status.value = {
        ...status.value,
        running: false,
        lastError: undefined,
        readiness: {
          state: "Pending",
          interfaces: [],
        },
        interfaces: [],
      };
      appendNodeControlEntry("Info", "Node stopped.");
      syncStatus.value = { ...EMPTY_SYNC_STATUS };
      clearAnnounceState();
      await refreshOperationalSummaryProjection();
      await refreshHubRegistrationState(false);

      for (const destination of Object.keys(discoveredByDestination)) {
        setPeerState(destination, "disconnected");
      }
    } catch (error: unknown) {
      throw captureActionError("Stop node failed", error);
    }
  }

  async function restartNode(): Promise<void> {
    try {
      await init();
      if (!client.value) {
        return;
      }
      clearLastError();
      clearReadinessError();
      await withTimeout(
        client.value.restart(toNodeConfig(settings)),
        NODE_START_TIMEOUT_MS,
        `node runtime restart timed out after ${NODE_START_TIMEOUT_MS}ms`,
      );
      setNodeConfigRestartRequired(false);
      await refreshStatusSnapshot(8, 250);
      applyRnodeInterfaceReadiness();
      await refreshMessagingState();
      await refreshAnnounceState();
      await refreshOperationalSummaryProjection();
      await configureClientLogging();
      await settleStartupDiscovery();
      void refreshHubRegistrationState(true).catch((error: unknown) => {
        appendNodeControlEntry(
          "Warn",
          `Hub registration bootstrap failed after restart: ${errorMessage(error)}`,
        );
      });
      appendNodeControlEntry("Info", "Node restarted with updated settings.");

    } catch (error: unknown) {
      throw captureRuntimeActionError("Restart node failed", error);
    }
  }

  async function connectPeer(destinationRaw: string): Promise<void> {
    await init();
    const destination = normalizeDestinationHex(destinationRaw);
    if (!isValidDestinationHex(destination)) {
      const message = `Invalid peer destination: ${destinationRaw}.`;
      appendLog("Debug", `[peers] connect blocked invalid-destination raw=${destinationRaw}.`);
      throw new Error(message);
    }
    if (!client.value) {
      const message = "Node client unavailable. Reinitialize the app and try again.";
      appendLog("Debug", `[peers] connect blocked destination=${destination}: client unavailable.`);
      throw new Error(message);
    }
    if (!status.value.running) {
      const message = "Start node before connecting to a peer.";
      setLastError(message);
      appendLog("Debug", `[peers] connect blocked destination=${destination}: node not running.`);
      throw new Error(message);
    }
    if (isLocalPeerDestination(destination)) {
      const message = `Cannot connect to local destination ${destination}.`;
      appendLog("Debug", `[peers] connect blocked self destination=${destination}.`);
      throw new Error(message);
    }
    const savedPeer = savedByDestination[destination];
    const activeTeamUid = hubDirectorySnapshot.value?.activeTeamUid
      || settings.teams.activeTeamUid;
    const isActiveTeamMember = hubDirectorySnapshot.value?.members.some(
      (member) => member.teamUid === activeTeamUid
        && normalizeDestinationHex(member.destinationHash) === destination,
    ) ?? false;
    if (!savedPeer && !isActiveTeamMember) {
      throw new Error(`Save peer ${destination} before connecting.`);
    }
    const discovered = peerByAnyKnownDestination(discoveredByDestination, destination);
    clearPeerRemoved(destination, discovered);

    try {
      clearLastError();
      logUi("Debug", `[peers] connect requested ${describePeerState(destination)}.`);
      const connectPromise = client.value.connectPeer(destination);
      markPeerManagedState(destination, true);
      await connectPromise;
      void settlePeerConnectionState(destination, "connected");
    } catch (error: unknown) {
      const message = errorMessage(error);
      setPeerState(destination, "disconnected", message);
      throw captureActionError(`Connect peer failed (${destination})`, error);
    }
  }

  async function disconnectPeer(destinationRaw: string): Promise<void> {
    await init();
    const destination = normalizeDestinationHex(destinationRaw);
    if (!isValidDestinationHex(destination)) {
      const message = `Invalid peer destination: ${destinationRaw}.`;
      appendLog("Debug", `[peers] disconnect blocked invalid-destination raw=${destinationRaw}.`);
      throw new Error(message);
    }
    if (!client.value) {
      const message = "Node client unavailable. Reinitialize the app and try again.";
      appendLog("Debug", `[peers] disconnect blocked destination=${destination}: client unavailable.`);
      throw new Error(message);
    }
    if (!status.value.running) {
      const message = "Start node before disconnecting a peer.";
      appendLog("Debug", `[peers] disconnect blocked destination=${destination}: node not running.`);
      throw new Error(message);
    }
    try {
      clearLastError();
      logUi("Debug", `[peers] disconnect requested ${describePeerState(destination)}.`);
      const disconnectPromise = client.value.disconnectPeer(destination);
      markPeerManagedState(destination, false);
      await disconnectPromise;
      await settlePeerConnectionState(destination, "disconnected");
      logUi("Debug", `[peers] disconnect applied ${describePeerState(destination)}.`);
    } catch (error: unknown) {
      throw captureActionError(`Disconnect peer failed (${destination})`, error);
    }
  }

  async function connectAllSaved(): Promise<void> {
    const results = await Promise.allSettled(
      Object.values(savedByDestination).map((peer) => connectPeer(peer.destination)),
    );
    const failures = results
      .filter((result): result is PromiseRejectedResult => result.status === "rejected")
      .map((result) => errorMessage(result.reason));
    if (failures.length > 0) {
      throw new Error(failures.join("; "));
    }
  }

  async function disconnectAllSaved(): Promise<void> {
    const results = await Promise.allSettled(
      Object.values(savedByDestination).map((peer) => disconnectPeer(peer.destination)),
    );
    const failures = results
      .filter((result): result is PromiseRejectedResult => result.status === "rejected")
      .map((result) => errorMessage(result.reason));
    if (failures.length > 0) {
      throw new Error(failures.join("; "));
    }
  }

  return {
    connectAllSaved,
    connectPeer,
    disconnectAllSaved,
    disconnectPeer,
    init,
    restartNode,
    startNode,
    stopNode,
  };
}
