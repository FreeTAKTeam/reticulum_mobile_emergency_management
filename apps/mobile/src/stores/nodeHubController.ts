import {
  type NodeStatus,
  type ReticulumNodeClient,
} from "@reticulum/node-client";
import type { Ref, ShallowRef } from "vue";

import {
  bootstrapHubRegistry,
  buildHubRegistryBootstrapProfile,
  clearHubRegistryLinkage,
  loadHubRegistryLinkage,
  matchesHubRegistryProfile,
  saveHubRegistryLinkage,
  type HubRegistryBootstrapProfile,
  type HubRegistryCommandTransport,
  type HubRegistryLinkage,
} from "../services/hubRegistryBootstrap";
import type { NodeUiSettings } from "../types/domain";
import { buildMissionCommandFieldsBase64 } from "../utils/missionSync";
import {
  hasSelectedHubIdentity,
  hubModeUsesRch,
} from "./nodeSettingsModel";
import {
  EMPTY_BYTES,
  type HubRegistrationSnapshot,
  type PacketSendOptions,
  asTrimmedString,
  nowMs,
} from "./nodeStoreCore";

interface NodeHubContext {
  appendLog: (level: string, message: string) => void;
  client: ShallowRef<ReticulumNodeClient | null>;
  errorMessage: (error: unknown) => string;
  hubRegistration: HubRegistrationSnapshot;
  sendBytes: (
    destination: string,
    bytes: Uint8Array,
    options?: PacketSendOptions,
  ) => Promise<void>;
  settings: NodeUiSettings;
  status: Ref<NodeStatus>;
}

export function createNodeHubController(context: NodeHubContext) {
  const {
    appendLog,
    client,
    errorMessage,
    hubRegistration,
    sendBytes,
    settings,
    status,
  } = context;
  let hubRegistryBootstrapInFlight: Promise<void> | null = null;

  function currentHubBootstrapProfile(): HubRegistryBootstrapProfile | null {
    if (!hubModeUsesRch(settings.hub.mode)) {
      return null;
    }
    if (!hasSelectedHubIdentity(settings.hub.identityHash)) {
      return null;
    }
    return buildHubRegistryBootstrapProfile({
      callsign: settings.displayName,
      localIdentityHex: status.value.identityHex,
      hubIdentityHash: settings.hub.identityHash,
    });
  }

  function setHubRegistrationPending(lastErrorValue?: string): void {
    hubRegistration.status = hubModeUsesRch(settings.hub.mode) ? "pending" : "disabled";
    if (lastErrorValue !== undefined) {
      hubRegistration.lastError = asTrimmedString(lastErrorValue);
    } else {
      hubRegistration.lastError = "";
    }
  }

  function setHubRegistrationReady(linkage: HubRegistryLinkage): void {
    hubRegistration.status = "ready";
    hubRegistration.linkage = { ...linkage };
    hubRegistration.lastReadyAt = nowMs();
    hubRegistration.lastError = "";
    saveHubRegistryLinkage(linkage);
  }

  function setHubRegistrationError(error: unknown): void {
    hubRegistration.status = hubModeUsesRch(settings.hub.mode) ? "error" : "disabled";
    hubRegistration.lastError = errorMessage(error);
    hubRegistration.lastAttemptAt = nowMs();
  }

  function clearHubRegistrationError(): void {
    if (hubRegistration.status === "error") {
      hubRegistration.status = "pending";
    }
    hubRegistration.lastError = "";
  }

  function reconcileHubRegistrationState(): void {
    if (!hubModeUsesRch(settings.hub.mode)) {
      hubRegistration.status = "disabled";
      hubRegistration.lastError = "";
      return;
    }

    if (!hasSelectedHubIdentity(settings.hub.identityHash)) {
      setHubRegistrationPending(
        settings.hub.mode === "Connected"
          ? "Connected mode requires selecting an RCH hub before outbound traffic can be routed."
          : "Select an RCH hub to seed peer routing from the hub directory.",
      );
      return;
    }

    const storedLinkage = loadHubRegistryLinkage();
    hubRegistration.linkage = storedLinkage ?? undefined;

    if (!storedLinkage) {
      setHubRegistrationPending("Hub registry linkage has not been established yet.");
      return;
    }

    const profile = currentHubBootstrapProfile();
    if (!profile) {
      setHubRegistrationPending("Hub registry bootstrap is waiting on a node identity and hub destination.");
      return;
    }

    if (matchesHubRegistryProfile(storedLinkage, profile)) {
      hubRegistration.status = "ready";
      hubRegistration.lastError = "";
      hubRegistration.lastReadyAt = storedLinkage.updatedAt ?? nowMs();
      return;
    }

    setHubRegistrationPending("Stored hub linkage does not match the current callsign, team color, or identity.");
  }

  function buildHubRegistryTransport(): HubRegistryCommandTransport {
    return {
      sendCommand: async (destinationHex: string, command) => {
        await sendBytes(destinationHex, EMPTY_BYTES, {
          fieldsBase64: buildMissionCommandFieldsBase64([command]),
        });
      },
      onPacket: (listener) => client.value?.on("packetReceived", listener) ?? (() => undefined),
    };
  }

  async function bootstrapHubRegistration(force = false): Promise<void> {
    if (!hubModeUsesRch(settings.hub.mode)) {
      reconcileHubRegistrationState();
      return;
    }

    if (hubRegistryBootstrapInFlight && !force) {
      return hubRegistryBootstrapInFlight;
    }

    const profile = currentHubBootstrapProfile();
    if (!profile) {
      setHubRegistrationPending(
        "Hub registry bootstrap is waiting on a callsign, node identity, or hub destination.",
      );
      return;
    }

    const storedLinkage = loadHubRegistryLinkage();
    if (!force && storedLinkage && matchesHubRegistryProfile(storedLinkage, profile)) {
      setHubRegistrationReady(storedLinkage);
      return;
    }

    if (!status.value.running) {
      setHubRegistrationPending("Hub registry bootstrap will run after the node is started.");
      return;
    }

    clearHubRegistrationError();
    hubRegistration.lastAttemptAt = nowMs();
    hubRegistration.lastError = "";
    hubRegistration.status = "pending";

    const transport = buildHubRegistryTransport();
    const bootstrapPromise = bootstrapHubRegistry(profile, transport)
      .then((linkage) => {
        setHubRegistrationReady(linkage);
        appendLog(
          "Info",
          `Hub registry linkage ready: team=${linkage.teamUid} member=${linkage.teamMemberUid}.`,
        );
      })
      .catch((error: unknown) => {
        setHubRegistrationError(error);
        throw error;
      })
      .finally(() => {
        hubRegistryBootstrapInFlight = null;
      });

    hubRegistryBootstrapInFlight = bootstrapPromise;
    return bootstrapPromise;
  }

  async function refreshHubRegistrationState(attemptBootstrap = false): Promise<void> {
    reconcileHubRegistrationState();
    if (!attemptBootstrap || !hubModeUsesRch(settings.hub.mode)) {
      return;
    }

    const profile = currentHubBootstrapProfile();
    if (!profile || !status.value.running) {
      return;
    }

    const storedLinkage = loadHubRegistryLinkage();
    if (storedLinkage && matchesHubRegistryProfile(storedLinkage, profile)) {
      setHubRegistrationReady(storedLinkage);
      return;
    }

    await bootstrapHubRegistration();
  }

  return {
    bootstrapHubRegistration,
    clearHubRegistrationError,
    currentHubBootstrapProfile,
    reconcileHubRegistrationState,
    refreshHubRegistrationState,
    setHubRegistrationPending,
  };
}
