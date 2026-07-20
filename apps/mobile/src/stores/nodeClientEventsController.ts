import {
  type AnnounceReceivedEvent,
  type AnnounceRecord,
  type HubDirectoryUpdatedEvent,
  type InterfaceStatusChangedEvent,
  type NodeLogEvent,
  type NodeErrorEvent,
  type NodeStatus,
  type PeerChangedEvent,
  type PeerRecord,
  type ProjectionInvalidationEvent,
  type ReticulumNodeClient,
  type SyncStatus,
  type StatusChangedEvent,
} from "@reticulum/node-client";
import type { Ref, ShallowRef } from "vue";

import type {
  HubDirectorySnapshot,
  NodeUiSettings,
} from "../types/domain";
import {
  logIndicatesReadinessError,
  nodeErrorIndicatesReadinessError,
} from "../utils/readinessErrors";
import { nativeLogShouldAppendToUi } from "../utils/nativeUiBackpressure";
import { normalizeDestinationHex } from "../utils/peers";
import { projectionRefreshCoordinator } from "../utils/projectionRefreshCoordinator";
import { hubModeUsesRch } from "./nodeSettingsModel";
import {
  EMPTY_STATUS,
  EMPTY_SYNC_STATUS,
  activePropagationNodeHex,
  advancePresenceNow,
  asTrimmedString,
  normalizeNodeStatus,
  sleep,
} from "./nodeStoreCore";

interface NodeClientEventsContext {
  appendLog: (level: string, message: string) => void;
  appendNodeControlEntry: (level: string, message: string, at?: number) => void;
  applyAnnounceUpdate: (
    event: AnnounceReceivedEvent | AnnounceRecord,
    source?: "live" | "snapshot",
  ) => void;
  applyPeerChanged: (change: PeerChangedEvent["change"]) => void;
  applyRnodeInterfaceReadiness: (at?: number) => void;
  client: ShallowRef<ReticulumNodeClient | null>;
  clearReadinessError: () => void;
  errorMessage: (error: unknown) => string;
  hubDirectorySnapshot: Ref<HubDirectorySnapshot | null>;
  isLocalDestinationIdentityPair: (destination: string, identity?: string) => boolean;
  lastError: Ref<string>;
  lastHubRefreshAt: Ref<number>;
  logUi: (level: string, message: string) => void;
  nodeErrorCanFallBackToConfiguredInterface: (event: NodeErrorEvent) => boolean;
  reconcileNativePeerSnapshot: (peers: PeerRecord[]) => void;
  refreshAnnounceState: () => Promise<void>;
  refreshHubRegistrationState: (attemptBootstrap?: boolean) => Promise<void>;
  refreshOperationalSummaryProjection: () => Promise<void>;
  refreshPluginProjection: (discover?: boolean) => Promise<void>;
  refreshPluginSensors: () => Promise<void>;
  refreshSavedPeersProjection: () => Promise<void>;
  refreshSettingsProjection: () => Promise<void>;
  scheduleOperationalSummaryRefresh: (delayMs?: number) => void;
  setReadinessError: (message: string, at?: number) => void;
  settings: NodeUiSettings;
  status: Ref<NodeStatus>;
  syncStatus: Ref<SyncStatus>;
  tcpInterfaceFailureCanFallBackToConfiguredInterface: (message: string) => boolean;
  telemetryDestinations: Ref<string[]>;
  unsubscribeClientEvents: Ref<Array<() => void>>;
  presenceNow: Ref<number>;
  upsertResolvedPeer: (peer: PeerRecord) => void;
  upsertNativeAnnounceRecord: (record: AnnounceRecord) => void;
}

export function createNodeClientEventsController(context: NodeClientEventsContext) {
  const {
    appendLog,
    appendNodeControlEntry,
    applyAnnounceUpdate,
    applyPeerChanged,
    applyRnodeInterfaceReadiness,
    client,
    clearReadinessError,
    errorMessage,
    hubDirectorySnapshot,
    isLocalDestinationIdentityPair,
    lastError,
    lastHubRefreshAt,
    logUi,
    nodeErrorCanFallBackToConfiguredInterface,
    reconcileNativePeerSnapshot,
    refreshAnnounceState,
    refreshHubRegistrationState,
    refreshOperationalSummaryProjection,
    refreshPluginProjection,
    refreshPluginSensors,
    refreshSavedPeersProjection,
    refreshSettingsProjection,
    scheduleOperationalSummaryRefresh,
    setReadinessError,
    settings,
    status,
    syncStatus,
    tcpInterfaceFailureCanFallBackToConfiguredInterface,
    telemetryDestinations,
    unsubscribeClientEvents,
    presenceNow,
    upsertResolvedPeer,
    upsertNativeAnnounceRecord,
  } = context;

  async function configureClientLogging(): Promise<void> {
    if (!client.value || !status.value.running) {
      return;
    }
    try {
      await client.value.setLogLevel("Info");
      appendLog("Debug", "Node client log level set to Info.");
    } catch (error: unknown) {
      logUi("Warn", `Failed to set node log level: ${errorMessage(error)}`);
    }
  }

  function resetClientEventBindings(): void {
    for (const unsubscribe of unsubscribeClientEvents.value) {
      unsubscribe();
    }
    unsubscribeClientEvents.value = [];
  }

  function bindClientEvents(nodeClient: ReticulumNodeClient): void {
    resetClientEventBindings();
    unsubscribeClientEvents.value = [
      nodeClient.on("statusChanged", (event: StatusChangedEvent) => {
        status.value = normalizeNodeStatus(event.status);
        const statusError = asTrimmedString(status.value.lastError);
        if (statusError && logIndicatesReadinessError(statusError)) {
          if (tcpInterfaceFailureCanFallBackToConfiguredInterface(statusError)) {
            clearReadinessError();
          } else {
            setReadinessError(statusError);
          }
        } else if (event.status.running && !statusError) {
          clearReadinessError();
        }
        applyRnodeInterfaceReadiness();
        void refreshHubRegistrationState(
          event.status.running && hubModeUsesRch(settings.hub.mode),
        ).catch((error: unknown) => {
          appendLog("Warn", `Hub registration status refresh failed: ${errorMessage(error)}`);
        });
      }),
      nodeClient.on("interfaceStatusChanged", (event: InterfaceStatusChangedEvent) => {
        const current = status.value.interfaces.filter(
          (entry) => entry.interfaceHex !== event.status.interfaceHex,
        );
        status.value = normalizeNodeStatus({
          ...status.value,
          interfaces: event.status.state === "disconnected" ? current : [...current, event.status],
        });
        applyRnodeInterfaceReadiness();
      }),
      nodeClient.on("announceReceived", (event: AnnounceReceivedEvent) => {
        upsertNativeAnnounceRecord(event);
        applyAnnounceUpdate(event, "live");
      }),
      nodeClient.on("peerChanged", (event: PeerChangedEvent) => {
        const destination = normalizeDestinationHex(event.change.destinationHex);
        if (isLocalDestinationIdentityPair(destination, event.change.identityHex)) {
          return;
        }
        presenceNow.value = advancePresenceNow(presenceNow.value);
        applyPeerChanged(event.change);
      }),
      nodeClient.on("peerResolved", (peer: PeerRecord) => {
        const destination = normalizeDestinationHex(peer.destinationHex);
        if (isLocalDestinationIdentityPair(destination, peer.identityHex)) {
          return;
        }
        presenceNow.value = advancePresenceNow(presenceNow.value, peer.lastSeenAtMs);
        upsertResolvedPeer(peer);
      }),
      nodeClient.on("hubDirectoryUpdated", (event: HubDirectoryUpdatedEvent) => {
        presenceNow.value = advancePresenceNow(presenceNow.value, event.receivedAtMs);
        hubDirectorySnapshot.value = {
          effectiveConnectedMode: event.effectiveConnectedMode,
          receivedAtMs: event.receivedAtMs,
          items: event.items.map((item) => ({
            ...item,
            announceCapabilities: [...item.announceCapabilities],
          })),
        };
        lastHubRefreshAt.value = event.receivedAtMs;
        void refreshMessagingState();
      }),
      nodeClient.on("operationalNotice", (event) => {
        appendNodeControlEntry(event.level, event.message, event.atMs);
      }),
      nodeClient.on("projectionInvalidated", (event: ProjectionInvalidationEvent) => {
        switch (event.scope) {
          case "AppSettings":
            void refreshSettingsProjection();
            break;
          case "SavedPeers":
            void refreshSavedPeersProjection();
            break;
          case "OperationalSummary":
            scheduleOperationalSummaryRefresh();
            break;
          case "Plugins":
            void refreshPluginProjection();
            break;
          case "PluginSensors":
            void refreshPluginSensors();
            break;
          default:
            break;
        }
      }),
      nodeClient.on("syncUpdated", (statusUpdate: SyncStatus) => {
        const previousRelay = activePropagationNodeHex(syncStatus.value);
        syncStatus.value = { ...statusUpdate };
        const nextRelay = activePropagationNodeHex(syncStatus.value);
        if (previousRelay !== nextRelay) {
          appendLog(
            "Debug",
            `[sync] propagation relay ${nextRelay ? `selected ${nextRelay}` : "cleared"}.`,
          );
        }
      }),
      nodeClient.on("log", (event: NodeLogEvent) => {
        if (logIndicatesReadinessError(event.message)) {
          if (tcpInterfaceFailureCanFallBackToConfiguredInterface(event.message)) {
            clearReadinessError();
          } else {
            setReadinessError(event.message);
          }
        }
        if (nativeLogShouldAppendToUi(event.level, event.message)) {
          appendLog(event.level, event.message);
        }
      }),
      nodeClient.on("error", (event: NodeErrorEvent) => {
        lastError.value = `${event.code}: ${event.message}`;
        if (nodeErrorIndicatesReadinessError(event)) {
          if (nodeErrorCanFallBackToConfiguredInterface(event)) {
            clearReadinessError();
          } else {
            setReadinessError(lastError.value);
          }
        }
        appendNodeControlEntry("Error", lastError.value);
      }),
    ];
  }

  async function refreshStatusSnapshot(
    retries = 1,
    delayMs = 250,
  ): Promise<NodeStatus> {
    if (!client.value) {
      return { ...EMPTY_STATUS };
    }

    let latest: NodeStatus = { ...EMPTY_STATUS };
    for (let attempt = 0; attempt < retries; attempt += 1) {
      try {
        latest = normalizeNodeStatus(await client.value.getStatus());
        status.value = { ...latest };
        if (latest.running || attempt === retries - 1) {
          return latest;
        }
      } catch {
        if (attempt === retries - 1) {
          status.value = { ...EMPTY_STATUS };
          return { ...EMPTY_STATUS };
        }
      }

      await sleep(delayMs);
    }

    return latest;
  }

  async function syncRuntimeSnapshot(reason: string): Promise<void> {
    const nextStatus = await refreshStatusSnapshot(2, 250);
    if (!nextStatus.running) {
      appendLog("Debug", `[startup] native runtime snapshot idle after ${reason}.`);
      return;
    }

    await refreshMessagingState();
    await refreshAnnounceState();
    await refreshOperationalSummaryProjection();
    await configureClientLogging();
    await refreshHubRegistrationState(hubModeUsesRch(settings.hub.mode));
    appendLog("Debug", `[startup] native runtime snapshot restored after ${reason}.`);
  }

  async function refreshMessagingState(): Promise<void> {
    if (!client.value || !status.value.running) {
      syncStatus.value = { ...EMPTY_SYNC_STATUS };
      telemetryDestinations.value = [];
      return;
    }

    await projectionRefreshCoordinator.run("node:messaging", async () => {
      const [peers, nextSyncStatus, nextTelemetryDestinations] = await Promise.all([
        client.value!.listPeers(),
        client.value!.getLxmfSyncStatus(),
        client.value!.listTelemetryDestinations(),
      ]);
      reconcileNativePeerSnapshot(peers);
      for (const peer of peers) {
        upsertResolvedPeer(peer);
      }
      syncStatus.value = { ...nextSyncStatus };
      telemetryDestinations.value = [...nextTelemetryDestinations];
    }).catch((error: unknown) => {
      appendLog("Debug", `Messaging projection refresh skipped: ${errorMessage(error)}`);
    });
  }

  return {
    bindClientEvents,
    configureClientLogging,
    refreshMessagingState,
    refreshStatusSnapshot,
    resetClientEventBindings,
    syncRuntimeSnapshot,
  };
}
