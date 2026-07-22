import {
  type NodeClientEvents,
  type NodeStatus,
  type ReticulumNodeClient,
  type SendMode,
  type SosAudioRecord,
} from "@reticulum/node-client";
import type {
  ComputedRef,
  Ref,
  ShallowRef,
} from "vue";

import type {
  DiscoveredPeer,
  NodeUiSettings,
} from "../types/domain";
import {
  hasCapability,
  normalizeDestinationHex,
} from "../utils/peers";
import { hasSelectedHubIdentity } from "./nodeSettingsModel";
import {
  EMPTY_STATUS,
  type PacketSendOptions,
  hasActualRemAnnounce,
} from "./nodeStoreCore";

interface NodeTransportContext {
  appendLog: (level: string, message: string) => void;
  bindClientEvents: (client: ReticulumNodeClient) => void;
  buildClient: () => ReticulumNodeClient;
  captureActionError: (action: string, error: unknown) => Error;
  clearAnnounceState: () => void;
  clearLastError: () => void;
  clearReadinessError: () => void;
  client: ShallowRef<ReticulumNodeClient | null>;
  configureClientLogging: () => Promise<void>;
  discoveredByDestination: Record<string, DiscoveredPeer>;
  initialized: Ref<boolean>;
  lastError: Ref<string>;
  logUi: (level: string, message: string) => void;
  peerByAnyKnownDestination: (
    peers: Record<string, DiscoveredPeer>,
    destination: string,
  ) => DiscoveredPeer | undefined;
  readinessErrorMessage: ComputedRef<string>;
  ready: ComputedRef<boolean>;
  refreshHubRegistrationState: (attemptBootstrap?: boolean) => Promise<void>;
  refreshOperationalSummaryProjection: () => Promise<void>;
  refreshSavedPeersProjection: () => Promise<void>;
  refreshSettingsProjection: () => Promise<void>;
  settings: NodeUiSettings;
  status: Ref<NodeStatus>;
}

export function createNodeTransportController(context: NodeTransportContext) {
  const {
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
    peerByAnyKnownDestination,
    readinessErrorMessage,
    ready,
    refreshHubRegistrationState,
    refreshOperationalSummaryProjection,
    refreshSavedPeersProjection,
    refreshSettingsProjection,
    settings,
    status,
  } = context;

  function notReadyMessage(action: string): string {
    if (readinessErrorMessage.value) {
      return `Cannot ${action} while the node is not ready: ${readinessErrorMessage.value}`;
    }
    return `Cannot ${action} until the node is ready. Wait for the top-right status to show Ready.`;
  }

  function assertReadyForOutbound(action: string): void {
    if (ready.value) {
      return;
    }

    const message = notReadyMessage(action);
    logUi(
      "Debug",
      `[ready] blocked outbound action=${action} running=${status.value.running} initialized=${initialized.value} readiness_error=${readinessErrorMessage.value || "none"}.`,
    );
    lastError.value = message;
    logUi("Warn", message);
    throw new Error(message);
  }

  function assertHubRoutingReadyForOutbound(action: string): void {
    if (settings.hub.mode !== "Connected") {
      return;
    }
    if (hasSelectedHubIdentity(settings.hub.identityHash)) {
      return;
    }

    const message = `Cannot ${action} until a connected-mode RCH hub is selected.`;
    lastError.value = message;
    logUi("Warn", message);
    throw new Error(message);
  }

  function destinationHasCapability(destinationRaw: string, capability: string): boolean {
    const peer = peerByAnyKnownDestination(discoveredByDestination, destinationRaw);
    if (!peer || !hasActualRemAnnounce(peer)) {
      return false;
    }
    return hasCapability(peer.appData ?? "", capability);
  }

  async function broadcastBytes(bytes: Uint8Array, options?: PacketSendOptions): Promise<void> {
    if (!client.value) {
      throw captureActionError("Broadcast failed", new Error("Node client is not initialized."));
    }
    try {
      assertReadyForOutbound("broadcast traffic");
      logUi(
        "Debug",
        `Broadcast requested bytes=${bytes.byteLength} fields=${options?.fieldsBase64 ? "lxmf" : "none"}.`,
      );
      await client.value.broadcastBytes(bytes, options);
    } catch (error: unknown) {
      throw captureActionError("Broadcast failed", error);
    }
  }

  async function sendBytes(
    destinationHex: string,
    bytes: Uint8Array,
    options?: PacketSendOptions,
  ): Promise<void> {
    const nodeClient = client.value;
    if (!nodeClient) {
      throw captureActionError(
        `Send failed (${destinationHex})`,
        new Error("Node client is not initialized."),
      );
    }
    try {
      assertReadyForOutbound("send traffic");
      assertHubRoutingReadyForOutbound("send traffic");
      const matchedPeer = peerByAnyKnownDestination(discoveredByDestination, destinationHex);
      const sendMode = options?.sendMode ?? "Auto";
      logUi(
        "Debug",
        `Send requested destination=${destinationHex} bytes=${bytes.byteLength} fields=${options?.fieldsBase64 ? "lxmf" : "none"} mode=${sendMode}${matchedPeer ? ` peer=${matchedPeer.label ?? matchedPeer.destination}` : ""}.`,
      );
      await nodeClient.sendBytes(destinationHex, bytes, {
        ...options,
        sendMode,
      });
      logUi(
        "Debug",
        `Send handed to native transport destination=${destinationHex} bytes=${bytes.byteLength} mode=${sendMode}.`,
      );
    } catch (error: unknown) {
      throw captureActionError(`Send failed (${destinationHex})`, error);
    }
  }

  async function sendBytesDirect(
    destinationHex: string,
    bytes: Uint8Array,
    options?: PacketSendOptions,
  ): Promise<void> {
    const nodeClient = client.value;
    if (!nodeClient) {
      throw captureActionError(
        `Direct send failed (${destinationHex})`,
        new Error("Node client is not initialized."),
      );
    }
    try {
      assertReadyForOutbound("send traffic");
      assertHubRoutingReadyForOutbound("send traffic");
      logUi(
        "Debug",
        `Direct send requested destination=${destinationHex} bytes=${bytes.byteLength} fields=${options?.fieldsBase64 ? "lxmf" : "none"}.`,
      );
      await nodeClient.sendBytes(destinationHex, bytes, {
        ...options,
        sendMode: "DirectOnly",
      });
      logUi(
        "Debug",
        `Direct send handed to native transport destination=${destinationHex} bytes=${bytes.byteLength}.`,
      );
    } catch (error: unknown) {
      throw captureActionError(`Direct send failed (${destinationHex})`, error);
    }
  }

  async function sendBytesViaPropagation(
    destinationHex: string,
    bytes: Uint8Array,
    options?: PacketSendOptions,
  ): Promise<void> {
    const nodeClient = client.value;
    if (!nodeClient) {
      throw captureActionError(
        `Propagation send failed (${destinationHex})`,
        new Error("Node client is not initialized."),
      );
    }
    try {
      assertReadyForOutbound("send traffic");
      assertHubRoutingReadyForOutbound("send traffic");
      logUi(
        "Debug",
        `Propagation send requested destination=${destinationHex} bytes=${bytes.byteLength} fields=${options?.fieldsBase64 ? "lxmf" : "none"}.`,
      );
      await nodeClient.sendBytes(destinationHex, bytes, {
        ...options,
        sendMode: "PropagationOnly",
      });
      logUi(
        "Debug",
        `Propagation send handed to native transport destination=${destinationHex} bytes=${bytes.byteLength}.`,
      );
    } catch (error: unknown) {
      throw captureActionError(`Propagation send failed (${destinationHex})`, error);
    }
  }

  async function sendLxmf(
    destinationHex: string,
    bodyUtf8: string,
    title?: string,
    options?: {
      sendMode?: SendMode;
    },
  ): Promise<string> {
    const nodeClient = client.value;
    if (!nodeClient) {
      throw captureActionError(
        `LXMF send failed (${destinationHex})`,
        new Error("Node client is not initialized."),
      );
    }
    try {
      assertReadyForOutbound("send LXMF");
      assertHubRoutingReadyForOutbound("send LXMF");
      const matchedPeer = peerByAnyKnownDestination(discoveredByDestination, destinationHex);
      const sendMode = options?.sendMode ?? "Auto";
      logUi(
        "Debug",
        `LXMF send requested destination=${destinationHex} bytes=${new TextEncoder().encode(bodyUtf8).byteLength} mode=${sendMode}${matchedPeer ? ` peer=${matchedPeer.label ?? matchedPeer.destination}` : ""}.`,
      );
      return await nodeClient.sendLxmf({
        destinationHex,
        bodyUtf8,
        title,
        sendMode,
      });
    } catch (error: unknown) {
      const captured = captureActionError(`LXMF send failed (${destinationHex})`, error);
      throw captured;
    }
  }

  async function retryLxmf(messageIdHex: string): Promise<void> {
    try {
      assertReadyForOutbound("retry LXMF");
      assertHubRoutingReadyForOutbound("retry LXMF");
      await requireClient(`LXMF retry failed (${messageIdHex})`).retryLxmf(messageIdHex);
    } catch (error: unknown) {
      throw captureActionError(`LXMF retry failed (${messageIdHex})`, error);
    }
  }

  function requireClient(action: string): ReticulumNodeClient {
    if (!client.value) {
      throw captureActionError(action, new Error("Node client is not initialized."));
    }
    return client.value;
  }

  function onClientEvent<K extends keyof NodeClientEvents>(
    event: K,
    handler: (payload: NodeClientEvents[K]) => void,
  ): () => void {
    return client.value?.on(event, handler) ?? (() => undefined);
  }

  async function getSosSettings() {
    return requireClient("Get SOS settings failed").getSosSettings();
  }

  async function setSosSettings(settingsRecord: Parameters<ReticulumNodeClient["setSosSettings"]>[0]): Promise<void> {
    await requireClient("Set SOS settings failed").setSosSettings(settingsRecord);
  }

  async function setSosPin(pin?: string): Promise<void> {
    await requireClient("Set SOS PIN failed").setSosPin(pin);
  }

  async function getSosStatus() {
    return requireClient("Get SOS status failed").getSosStatus();
  }

  async function triggerSos(source?: Parameters<ReticulumNodeClient["triggerSos"]>[0]) {
    return requireClient("Trigger SOS failed").triggerSos(source);
  }

  async function deactivateSos(pin?: string) {
    return requireClient("Deactivate SOS failed").deactivateSos(pin);
  }

  async function submitSosTelemetry(telemetry: Parameters<ReticulumNodeClient["submitSosTelemetry"]>[0]): Promise<void> {
    await requireClient("Submit SOS telemetry failed").submitSosTelemetry(telemetry);
  }

  async function listSosAlerts() {
    return requireClient("List SOS alerts failed").listSosAlerts();
  }

  async function listSosLocations() {
    return requireClient("List SOS locations failed").listSosLocations();
  }

  async function listSosAudio() {
    return requireClient("List SOS audio failed").listSosAudio();
  }

  async function recordSosAudio(audio: SosAudioRecord) {
    return requireClient("Record SOS audio failed").recordSosAudio(audio);
  }

  async function announceNow(): Promise<void> {
    if (!client.value) {
      return;
    }
    try {
      await client.value.announceNow();
    } catch (error: unknown) {
      throw captureActionError("Announce now failed", error);
    }
  }

  async function requestPeerIdentity(destinationHex: string): Promise<void> {
    if (!client.value) {
      return;
    }
    try {
      await client.value.requestPeerIdentity(destinationHex);
    } catch (error: unknown) {
      throw captureActionError(`Peer identity request failed (${destinationHex})`, error);
    }
  }

  async function setActivePropagationNode(destinationHex?: string): Promise<void> {
    if (!client.value) {
      return;
    }
    try {
      await client.value.setActivePropagationNode(destinationHex);
    } catch (error: unknown) {
      throw captureActionError("Set active propagation node failed", error);
    }
  }

  async function requestLxmfSync(limit?: number): Promise<void> {
    if (!client.value) {
      return;
    }
    try {
      await client.value.requestLxmfSync(limit);
    } catch (error: unknown) {
      throw captureActionError("LXMF sync request failed", error);
    }
  }

  async function broadcastJson(payload: unknown): Promise<void> {
    const body = new TextEncoder().encode(JSON.stringify(payload));
    await broadcastBytes(body);
  }

  async function sendJson(
    destinationHex: string,
    payload: unknown,
  ): Promise<void> {
    const body = new TextEncoder().encode(JSON.stringify(payload));
    await sendBytes(destinationHex, body);
  }

  async function reinitializeClient(): Promise<void> {
    try {
      clearLastError();
      clearReadinessError();
      if (client.value) {
        await client.value.dispose().catch(() => undefined);
      }
      client.value = buildClient();
      bindClientEvents(client.value);
      await configureClientLogging();
      status.value = { ...EMPTY_STATUS };
      clearAnnounceState();
      await Promise.all([
        refreshSettingsProjection(),
        refreshSavedPeersProjection(),
        refreshOperationalSummaryProjection(),
      ]);
      await refreshHubRegistrationState(false);
      appendLog("Info", "Node client recreated.");
    } catch (error: unknown) {
      throw captureActionError("Recreate client failed", error);
    }
  }

  return {
    announceNow,
    assertHubRoutingReadyForOutbound,
    assertReadyForOutbound,
    broadcastBytes,
    broadcastJson,
    deactivateSos,
    destinationHasCapability,
    getSosSettings,
    getSosStatus,
    listSosAlerts,
    listSosAudio,
    listSosLocations,
    onClientEvent,
    recordSosAudio,
    reinitializeClient,
    requestLxmfSync,
    requestPeerIdentity,
    retryLxmf,
    sendBytes,
    sendBytesDirect,
    sendBytesViaPropagation,
    sendJson,
    sendLxmf,
    setActivePropagationNode,
    setSosPin,
    setSosSettings,
    submitSosTelemetry,
    triggerSos,
  };
}
