import { createReticulumNodeClient, type ReticulumNodeClient } from "@reticulum/node-client";
import { defineStore } from "pinia";

import { loadUiSettingsProjection } from "../utils/legacyState";
import { runtimeProfile } from "../utils/runtimeProfile";
import { createNodeActionsController } from "./nodeActionsController";
import { createNodeAnnounceController } from "./nodeAnnounceController";
import { createNodeClientEventsController } from "./nodeClientEventsController";
import { createNodeHubController } from "./nodeHubController";
import { createNodeLifecycleController } from "./nodeLifecycleController";
import { createNodeLoggingController } from "./nodeLoggingController";
import { createNodePeerController } from "./nodePeerController";
import { createNodePeerSelectors } from "./nodePeerSelectors";
import { createNodeProjectionController } from "./nodeProjectionController";
import { DEFAULT_SETTINGS } from "./nodeSettingsModel";
import { createNodeStoreApi } from "./nodeStoreApi";
import { createNodeStoreState } from "./nodeStoreState";
import { createNodeTransportController } from "./nodeTransportController";

export const useNodeStore = defineStore("node", () => {
  const state = createNodeStoreState();
  const {
    announceByDestination, appDestinationByIdentity, client, discoveredByDestination,
    hubDirectorySnapshot, hubRegistration, initialized, lastError, lastHubRefreshAt,
    liveLxmfPresenceByIdentity, livePresenceByDestination, logs, lxmfDestinationByIdentity,
    nodeConfigRestartRequired, nodeControlEntries, operationalSummary, pluginSensors, plugins,
    presenceNow, readinessError, removedByDestination, savedByDestination, settings,
    startupSettling, status, syncStatus, telemetryDestinations, unsubscribeClientEvents,
  } = state;

  const logging = createNodeLoggingController({
    client,
    lastError,
    logs,
    nodeConfigRestartRequired,
    nodeControlEntries,
    readinessError,
    settings,
    status,
  });
  const {
    appendLog,
    appendNodeControlEntry,
    applyRnodeInterfaceReadiness,
    captureActionError,
    captureRuntimeActionError,
    clearLastError,
    clearReadinessError,
    defaultsWithTcpFallback,
    errorMessage,
    logUi,
    nodeErrorCanFallBackToConfiguredInterface,
    setLastError,
    setNodeConfigRestartRequired,
    setReadinessError,
    tcpInterfaceFailureCanFallBackToConfiguredInterface,
  } = logging;

  const peers = createNodePeerController({
    announceByDestination,
    appDestinationByIdentity,
    discoveredByDestination,
    hubDirectorySnapshot,
    lastHubRefreshAt,
    lxmfDestinationByIdentity,
    nativeSavedPeerForCanonicalDestination: (...args) => nativeSavedPeerForCanonicalDestination(...args),
    refreshSavedPeerProfile: (destination, reason) => refreshSavedPeerProfile(destination, reason),
    refreshMessagingState: () => refreshMessagingState(),
    savedByDestination,
    status,
  });
  const {
    applyPeerChanged,
    clearAnnounceState,
    clearHubDirectoryState,
    describePeerState,
    isLocalDestinationIdentityPair,
    isLocalPeer,
    isLocalPeerDestination,
    markPeerManagedState,
    reconcileNativePeerSnapshot,
    setPeerState,
    settlePeerConnectionState,
    upsertDiscovered,
    upsertNativeAnnounceRecord,
    upsertResolvedPeer,
  } = peers;

  const projections = createNodeProjectionController({
    appendLog,
    client,
    defaultsWithTcpFallback,
    discoveredByDestination,
    errorMessage,
    init: () => init(),
    lastError,
    operationalSummary,
    plugins,
    pluginSensors,
    savedByDestination,
    settings,
    status,
    upsertDiscovered,
  });
  const {
    applyUiSettingsProjection,
    importLegacyProjectionState,
    persistSavedPeersProjection,
    persistSettingsProjection,
    refreshOperationalSummaryProjection,
    refreshPluginProjection,
    refreshPluginSensors,
    refreshSavedPeersProjection,
    refreshSettingsProjection,
    scheduleOperationalSummaryRefresh,
  } = projections;

  applyUiSettingsProjection(loadUiSettingsProjection(DEFAULT_SETTINGS));

  const announce = createNodeAnnounceController({
    appendLog,
    appDestinationByIdentity,
    client,
    discoveredByDestination,
    liveLxmfPresenceByIdentity,
    livePresenceByDestination,
    lxmfDestinationByIdentity,
    errorMessage,
    isLocalDestinationIdentityPair,
    peerByAnyKnownDestination: (peers, destination) => peerByAnyKnownDestination(peers, destination),
    persistSavedPeersProjection,
    presenceNow,
    savedByDestination,
    setLastError,
    startupSettling,
    status,
    refreshMessagingState: () => refreshMessagingState(),
    upsertDiscovered,
    upsertNativeAnnounceRecord,
  });
  const {
    applyAnnounceUpdate,
    nativeSavedPeerForCanonicalDestination,
    refreshAnnounceState,
    refreshSavedPeerProfile,
    savedPeerProfileFromDiscovered,
    settleStartupDiscovery,
  } = announce;

  function buildClient(): ReticulumNodeClient {
    if (runtimeProfile === "web") {
      return createReticulumNodeClient({
        mode: "web",
      });
    }
    return createReticulumNodeClient({
      mode: settings.clientMode,
    });
  }

  const hub = createNodeHubController({
    appendLog,
    client,
    errorMessage,
    hubDirectorySnapshot,
    hubRegistration,
    settings,
    status,
  });
  const {
    currentHubBootstrapProfile,
    refreshHubRegistrationState,
    setHubRegistrationPending,
  } = hub;

  const {
    bindClientEvents,
    configureClientLogging,
    refreshMessagingState,
    refreshStatusSnapshot,
    syncRuntimeSnapshot,
  } = createNodeClientEventsController({
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
  });

  const selectors = createNodePeerSelectors({
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
    settings,
    status,
    syncStatus,
  });
  const {
    clearPeerRemoved,
    markPeerRemoved,
    peerByAnyKnownDestination,
    readinessErrorMessage,
    ready,
  } = selectors;

  const lifecycle = createNodeLifecycleController({
    appendLog,
    appendNodeControlEntry,
    applyRnodeInterfaceReadiness,
    bindClientEvents,
    buildClient,
    captureActionError,
    captureRuntimeActionError,
    clearAnnounceState,
    clearLastError,
    clearPeerRemoved: (destination, peer) => clearPeerRemoved(destination, peer),
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
  });
  const { init } = lifecycle;

  const actions = createNodeActionsController({
    appendLog,
    captureActionError,
    clearHubDirectoryState,
    clearLastError,
    clearPeerRemoved: (destination, peer) => clearPeerRemoved(destination, peer),
    client,
    defaultsWithTcpFallback,
    discoveredByDestination,
    errorMessage,
    hubRegistration,
    init,
    lastError,
    markPeerRemoved: (destination, peer) => markPeerRemoved(destination, peer),
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
  });
  const transport = createNodeTransportController({
    appendLog,
    bindClientEvents,
    buildClient,
    captureActionError,
    clearAnnounceState,
    clearLastError,
    clearReadinessError,
    client,
    configureClientLogging,
    discoveredByDestination,
    initialized,
    lastError,
    logUi,
    peerByAnyKnownDestination: (peers, destination) => peerByAnyKnownDestination(peers, destination),
    readinessErrorMessage,
    ready,
    refreshHubRegistrationState,
    refreshOperationalSummaryProjection,
    refreshSavedPeersProjection,
    refreshSettingsProjection,
    settings,
    status,
  });
  return createNodeStoreApi(
    state, logging, peers, projections, hub, selectors, lifecycle, actions, transport,
  );
});
