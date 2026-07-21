import {
  YELLOW_TEAM_UID,
  type NodeStatus,
  type ReticulumNodeClient,
} from "@reticulum/node-client";
import type { Ref, ShallowRef } from "vue";

import {
  buildHubRegistryBootstrapProfile,
  saveHubRegistryLinkage,
  type HubRegistryBootstrapProfile,
  type HubRegistryLinkage,
} from "../services/hubRegistryBootstrap";
import type { HubDirectorySnapshot, NodeUiSettings } from "../types/domain";
import {
  hasSelectedHubIdentity,
  hubModeUsesRch,
} from "./nodeSettingsModel";
import {
  type HubRegistrationSnapshot,
  asTrimmedString,
  nowMs,
} from "./nodeStoreCore";

interface NodeHubContext {
  appendLog: (level: string, message: string) => void;
  client: ShallowRef<ReticulumNodeClient | null>;
  errorMessage: (error: unknown) => string;
  hubDirectorySnapshot: Ref<HubDirectorySnapshot | null>;
  hubRegistration: HubRegistrationSnapshot;
  settings: NodeUiSettings;
  status: Ref<NodeStatus>;
}

export function createNodeHubController(context: NodeHubContext) {
  const {
    appendLog,
    client,
    errorMessage,
    hubDirectorySnapshot,
    hubRegistration,
    settings,
    status,
  } = context;
  let directoryRefreshInFlight: Promise<void> | null = null;

  function currentHubBootstrapProfile(): HubRegistryBootstrapProfile | null {
    if (!hubModeUsesRch(settings.hub.mode) || !hasSelectedHubIdentity(settings.hub.identityHash)) {
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
    hubRegistration.lastError = asTrimmedString(lastErrorValue);
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
      hubRegistration.linkage = undefined;
      hubRegistration.lastError = "";
      return;
    }
    if (!hasSelectedHubIdentity(settings.hub.identityHash)) {
      setHubRegistrationPending("Select an RCH hub before loading the read-only TEAM directory.");
      return;
    }
    const snapshot = hubDirectorySnapshot.value;
    const configuredHub = settings.hub.identityHash.trim().toLowerCase();
    if (!snapshot || snapshot.hubIdentityHash?.toLowerCase() !== configuredHub) {
      setHubRegistrationPending("Waiting for the selected RCH hub TEAM directory.");
      return;
    }
    const activeTeamUid = snapshot.activeTeamUid || settings.teams.activeTeamUid || YELLOW_TEAM_UID;
    const membership = snapshot.callerMemberships.find((item) => item.teamUid === activeTeamUid)
      ?? (activeTeamUid === YELLOW_TEAM_UID && snapshot.schemaVersion === 0
        ? { teamUid: YELLOW_TEAM_UID, teamMemberUid: status.value.appDestinationHex }
        : undefined);
    if (!membership?.teamMemberUid) {
      if (settings.teams.localTeams.some((team) => team.teamUid === activeTeamUid)) {
        hubRegistration.status = "ready";
        hubRegistration.linkage = undefined;
        hubRegistration.lastReadyAt = nowMs();
        hubRegistration.lastError = "";
        return;
      }
      hubRegistration.status = "error";
      hubRegistration.linkage = undefined;
      hubRegistration.lastError = "RCH has not assigned this REM client to the active TEAM. Ask an RCH operator to add the membership.";
      return;
    }
    const teamColor = snapshot.teams.find((team) => team.uid === membership.teamUid)?.color ?? "YELLOW";
    const linkage: HubRegistryLinkage = {
      teamUid: membership.teamUid,
      teamMemberUid: membership.teamMemberUid,
      callsign: settings.displayName,
      teamColor: teamColor as HubRegistryLinkage["teamColor"],
      localIdentityHex: status.value.identityHex,
      hubIdentityHash: configuredHub,
      updatedAt: nowMs(),
    };
    hubRegistration.status = "ready";
    hubRegistration.linkage = linkage;
    hubRegistration.lastReadyAt = linkage.updatedAt;
    hubRegistration.lastError = "";
    saveHubRegistryLinkage(linkage);
  }

  async function refreshHubRegistrationState(refreshDirectory = false): Promise<void> {
    reconcileHubRegistrationState();
    if (!refreshDirectory || !hubModeUsesRch(settings.hub.mode) || !status.value.running) {
      return;
    }
    const nodeClient = client.value;
    if (!nodeClient || !hasSelectedHubIdentity(settings.hub.identityHash)) {
      return;
    }
    if (directoryRefreshInFlight) {
      return directoryRefreshInFlight;
    }
    hubRegistration.lastAttemptAt = nowMs();
    directoryRefreshInFlight = (async () => {
      try {
        await nodeClient.refreshHubDirectory();
        const snapshot = await nodeClient.getHubDirectorySnapshot();
        hubDirectorySnapshot.value = {
          ...snapshot,
          teams: snapshot.teams.map((team) => ({ ...team })),
          callerMemberships: snapshot.callerMemberships.map((item) => ({ ...item })),
          members: snapshot.members.map((member) => ({
            ...member,
            announceCapabilities: [...member.announceCapabilities],
          })),
          localTeams: snapshot.localTeams.map((team) => ({
            ...team,
            memberDestinations: [...team.memberDestinations],
          })),
          items: snapshot.items.map((item) => ({
            ...item,
            announceCapabilities: [...item.announceCapabilities],
          })),
        };
        reconcileHubRegistrationState();
      } catch (error: unknown) {
        hubRegistration.status = "error";
        hubRegistration.lastError = errorMessage(error);
        appendLog("Warn", `Read-only RCH TEAM directory refresh failed: ${hubRegistration.lastError}`);
        throw error;
      } finally {
        directoryRefreshInFlight = null;
      }
    })();
    return directoryRefreshInFlight;
  }

  async function bootstrapHubRegistration(force = false): Promise<void> {
    await refreshHubRegistrationState(force || !hubDirectorySnapshot.value);
  }

  async function setActiveTeam(teamUid: string): Promise<void> {
    const normalized = teamUid.trim().toLowerCase();
    const nodeClient = client.value;
    if (!nodeClient) {
      throw new Error("Node client is not initialized.");
    }
    await nodeClient.setActiveTeam(normalized);
    settings.teams.activeTeamUid = normalized;
    if (hubDirectorySnapshot.value) {
      hubDirectorySnapshot.value.activeTeamUid = normalized;
    }
    reconcileHubRegistrationState();
  }

  return {
    bootstrapHubRegistration,
    clearHubRegistrationError,
    currentHubBootstrapProfile,
    reconcileHubRegistrationState,
    refreshHubRegistrationState,
    setActiveTeam,
    setHubRegistrationPending,
  };
}
