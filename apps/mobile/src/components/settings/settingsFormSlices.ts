import type { PreferredMapLayer, RnodeConnectionMode } from "@reticulum/node-client";

import type { NodeUiSettings } from "../../types/domain";
import { householdIdFromDestination } from "../../utils/communityStatus";
import { ensureRequiredAnnounceCapabilities } from "../../utils/peers";
import {
  RNODE_FREQUENCY_MAX_HZ,
  RNODE_FREQUENCY_MIN_HZ,
  isRnodeFrequencyHz,
  normalizeRnodeSettings,
} from "../../utils/rnodeProfiles";
import type { NodeSettingsSlice } from "./useNodeSettingsForm";

/*
 * Draft <-> settings mapping for each Settings form page. Normalization here is
 * input clamping the old single form already did; the runtime validates again.
 */

function splitRoleBadges(text: string): string[] {
  return text.split(",").map((role) => role.trim()).filter(Boolean);
}

export interface IdentityDraft {
  displayName: string;
  householdId: string;
  householdName: string;
  adults: number;
  children: number;
  pets: number;
  roleBadges: string[];
  /** Text typed into the role badge input but not yet turned into a chip. */
  roleBadgeText: string;
  announceCapabilities: string;
  announceIntervalSeconds: number;
}

export const identitySlice: NodeSettingsSlice<IdentityDraft> = {
  read: (settings, appDestinationHex) => ({
    displayName: settings.displayName,
    householdId: settings.community.householdId || householdIdFromDestination(appDestinationHex),
    householdName: settings.community.householdName,
    adults: settings.community.adults,
    children: settings.community.children,
    pets: settings.community.pets,
    roleBadges: [...settings.community.roleBadges],
    roleBadgeText: "",
    announceCapabilities: ensureRequiredAnnounceCapabilities(settings.announceCapabilities),
    announceIntervalSeconds: settings.announceIntervalSeconds,
  }),
  patch: (draft, settings) => ({
    displayName: draft.displayName,
    announceCapabilities: ensureRequiredAnnounceCapabilities(draft.announceCapabilities.trim()),
    announceIntervalSeconds: Math.max(60, Number(draft.announceIntervalSeconds || 1800)),
    community: {
      ...settings.community,
      householdId: draft.householdId.trim().toLowerCase(),
      householdName: draft.householdName.trim(),
      adults: Math.min(20, Math.max(0, Number(draft.adults))),
      children: Math.min(20, Math.max(0, Number(draft.children))),
      pets: Math.min(20, Math.max(0, Number(draft.pets))),
      roleBadges: [...new Set([...draft.roleBadges, ...splitRoleBadges(draft.roleBadgeText)])].slice(0, 5),
    },
  }),
};

export interface NetworkDraft {
  tcpClients: string[];
  broadcast: boolean;
  transportNodeEnabled: boolean;
  rnodeEnabled: boolean;
  rnodeConnectionMode: RnodeConnectionMode;
  rnodePeripheralId: string;
  rnodeDisplayName: string;
  rnodeRegion: string;
  rnodeProfile: string;
  rnodeFrequencyHz: number;
}

export function normalizedTcpClients(entries: readonly string[]): string[] {
  return [...new Set(entries.map((entry) => entry.trim()).filter((entry) => entry.length > 0))];
}

export function draftRnodeSettings(draft: NetworkDraft, settings: NodeUiSettings) {
  return normalizeRnodeSettings(
    {
      enabled: draft.rnodeEnabled,
      connectionMode: draft.rnodeConnectionMode,
      peripheralId: draft.rnodePeripheralId,
      displayName: draft.rnodeDisplayName,
      region: draft.rnodeRegion,
      profile: draft.rnodeProfile,
      frequencyHz: draft.rnodeFrequencyHz,
    },
    settings.rnode,
  );
}

export const networkSlice: NodeSettingsSlice<NetworkDraft> = {
  read: (settings) => ({
    tcpClients: [...settings.tcpClients],
    broadcast: settings.broadcast,
    transportNodeEnabled: settings.transportNodeEnabled,
    rnodeEnabled: settings.rnode.enabled,
    rnodeConnectionMode: settings.rnode.connectionMode,
    rnodePeripheralId: settings.rnode.peripheralId,
    rnodeDisplayName: settings.rnode.displayName,
    rnodeRegion: settings.rnode.region,
    rnodeProfile: settings.rnode.profile,
    rnodeFrequencyHz: settings.rnode.frequencyHz,
  }),
  patch: (draft, settings) => ({
    tcpClients: normalizedTcpClients(draft.tcpClients),
    broadcast: draft.broadcast,
    transportNodeEnabled: draft.transportNodeEnabled,
    rnode: draftRnodeSettings(draft, settings),
  }),
  validate: (draft) => (
    isRnodeFrequencyHz(draft.rnodeFrequencyHz)
      ? undefined
      : `RNode LoRa frequency must be between ${RNODE_FREQUENCY_MIN_HZ} and ${RNODE_FREQUENCY_MAX_HZ} Hz.`
  ),
};

export interface RoutingDraft {
  hubMode: NodeUiSettings["hub"]["mode"];
  hubIdentityHash: string;
  hubApiBaseUrl: string;
  hubApiKey: string;
  hubRefreshIntervalSeconds: number;
}

export const routingSlice: NodeSettingsSlice<RoutingDraft> = {
  read: (settings) => ({
    hubMode: settings.hub.mode,
    hubIdentityHash: settings.hub.identityHash,
    hubApiBaseUrl: settings.hub.apiBaseUrl,
    hubApiKey: settings.hub.apiKey,
    hubRefreshIntervalSeconds: settings.hub.refreshIntervalSeconds,
  }),
  patch: (draft) => ({
    hub: {
      mode: draft.hubMode,
      identityHash: draft.hubIdentityHash.trim(),
      apiBaseUrl: draft.hubApiBaseUrl.trim(),
      apiKey: draft.hubApiKey.trim(),
      refreshIntervalSeconds: Math.max(30, Number(draft.hubRefreshIntervalSeconds || 3600)),
    },
  }),
};

export interface LocationDraft {
  enabled: boolean;
  publishIntervalSeconds: number;
  accuracyThresholdMeters: number | undefined;
  staleAfterMinutes: number;
  expireAfterMinutes: number;
}

function positiveOrDefault(value: number | string | undefined | null, fallback: number): number {
  const parsed = Number(value || fallback);
  return Number.isFinite(parsed) ? Math.max(1, parsed) : fallback;
}

export const locationSlice: NodeSettingsSlice<LocationDraft> = {
  read: (settings) => ({
    enabled: settings.telemetry.enabled,
    publishIntervalSeconds: settings.telemetry.publishIntervalSeconds,
    accuracyThresholdMeters: settings.telemetry.accuracyThresholdMeters,
    staleAfterMinutes: settings.telemetry.staleAfterMinutes,
    expireAfterMinutes: settings.telemetry.expireAfterMinutes,
  }),
  patch: (draft) => {
    const accuracy = draft.accuracyThresholdMeters;
    return {
      telemetry: {
        enabled: draft.enabled,
        publishIntervalSeconds: positiveOrDefault(draft.publishIntervalSeconds, 60),
        accuracyThresholdMeters: accuracy === undefined || accuracy === null || accuracy === 0
          ? undefined
          : Math.max(1, Number(accuracy)),
        staleAfterMinutes: positiveOrDefault(draft.staleAfterMinutes, 30),
        expireAfterMinutes: positiveOrDefault(draft.expireAfterMinutes, 180),
      },
    };
  },
  validate: (draft) => (
    positiveOrDefault(draft.expireAfterMinutes, 180) < positiveOrDefault(draft.staleAfterMinutes, 30)
      ? "Hide time must be the same as or longer than the stale time."
      : undefined
  ),
};

export interface MapDraft {
  preferredMapLayer: PreferredMapLayer;
}

export const mapSlice: NodeSettingsSlice<MapDraft> = {
  read: (settings) => ({ preferredMapLayer: settings.community.preferredMapLayer }),
  patch: (draft, settings) => ({
    community: { ...settings.community, preferredMapLayer: draft.preferredMapLayer },
  }),
};

export interface PowerDraft {
  enabled: boolean;
  thresholdPercent: NodeUiSettings["power"]["thresholdPercent"];
}

export const powerSlice: NodeSettingsSlice<PowerDraft> = {
  read: (settings) => ({ ...settings.power }),
  patch: (draft) => ({ power: { enabled: draft.enabled, thresholdPercent: draft.thresholdPercent } }),
};
