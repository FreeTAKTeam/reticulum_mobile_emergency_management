import {
  PhBatteryCharging,
  PhCompass,
  PhMapPin,
  PhMapTrifold,
  PhPlugsConnected,
  PhPuzzlePiece,
  PhQrCode,
  PhUser,
  PhUsersThree,
  PhWarning,
  PhWrench,
} from "@phosphor-icons/vue";
import { computed, type Component } from "vue";
import type { RouteLocationRaw } from "vue-router";

import { useTeamDirectory } from "../../composables/useTeamDirectory";
import { useNodeStore } from "../../stores/nodeStore";
import { useSosStore } from "../../stores/sosStore";
import { useTelemetryStore } from "../../stores/telemetryStore";
import {
  foldSummary,
  identitySummary,
  locationSummary,
  mapSummary,
  networkSummary,
  nodeSummary,
  pluginsSummary,
  powerSummary,
  routingSummary,
  sosSummary,
  teamsSummary,
} from "../../utils/settingsIndexSummaries";
import { normalizedTcpClients } from "./settingsFormSlices";

export interface SettingsIndexRow {
  id: string;
  label: string;
  summary: string;
  icon: Component;
  to: RouteLocationRaw;
  /** Task rows name an action rather than a setting value. */
  task?: boolean;
}

export interface SettingsIndexSection {
  id: string;
  label: string;
  rows: SettingsIndexRow[];
  foldedSummary: string;
}

function section(id: string, label: string, rows: SettingsIndexRow[]): SettingsIndexSection {
  return { id, label, rows, foldedSummary: foldSummary(rows.filter((row) => !row.task).map((row) => row.summary)) };
}

/** The Settings index, grouped by user goal, with each row summarizing its current state. */
export function useSettingsIndexSections() {
  const nodeStore = useNodeStore();
  const sosStore = useSosStore();
  const telemetryStore = useTelemetryStore();
  const { teamSections } = useTeamDirectory();

  return computed<SettingsIndexSection[]>(() => {
    const settings = nodeStore.settings;
    const teams = teamSections.value;
    const sos = sosStore.settings;
    const enabledPlugins = nodeStore.plugins.filter((plugin) => plugin.enabled).length;
    return [
      section("you", "You", [
        {
          id: "identity",
          label: "Identity",
          summary: identitySummary(settings.displayName, settings.community.householdName),
          icon: PhUser,
          to: { name: "settings-identity" },
        },
        {
          id: "teams",
          label: "Teams",
          summary: teamsSummary(
            teams.filter((entry) => entry.local).length,
            teams.filter((entry) => !entry.local && entry.rch).length,
          ),
          icon: PhUsersThree,
          to: { name: "manage-teams" },
        },
      ]),
      section("connectivity", "Connectivity", [
        {
          id: "network",
          label: "Network interfaces",
          summary: networkSummary(normalizedTcpClients(settings.tcpClients).length, settings.rnode.enabled),
          icon: PhPlugsConnected,
          to: { name: "settings-network" },
        },
        {
          id: "routing",
          label: "Routing & hubs",
          summary: routingSummary(settings.hub.mode, settings.hub.identityHash),
          icon: PhCompass,
          to: { name: "settings-routing" },
        },
      ]),
      section("sharing", "Sharing", [
        {
          id: "location",
          label: "Location sharing",
          summary: locationSummary(
            settings.telemetry.enabled,
            settings.telemetry.publishIntervalSeconds,
            telemetryStore.loopStatus,
          ),
          icon: PhMapPin,
          to: { name: "settings-location" },
        },
        {
          id: "map",
          label: "Map",
          summary: mapSummary(settings.community.preferredMapLayer),
          icon: PhMapTrifold,
          to: { name: "settings-map" },
        },
      ]),
      section("emergency", "Emergency", [
        {
          id: "sos",
          label: "SOS",
          summary: sosSummary(sos.enabled, {
            shake: sos.triggerShake,
            powerButton: sos.triggerPowerButton,
            floatingButton: sos.floatingButton,
          }),
          icon: PhWarning,
          to: { name: "settings-sos" },
        },
      ]),
      section("device", "Device", [
        {
          id: "power",
          label: "Battery saver",
          summary: powerSummary(settings.power.enabled, settings.power.thresholdPercent),
          icon: PhBatteryCharging,
          to: { name: "settings-power" },
        },
        {
          id: "plugins",
          label: "Plugins",
          summary: pluginsSummary(nodeStore.plugins.length, enabledPlugins),
          icon: PhPuzzlePiece,
          to: { name: "settings-plugins" },
        },
        {
          id: "share-setup",
          label: "Share this setup",
          summary: "Create or scan a Block Code",
          icon: PhQrCode,
          to: { name: "settings-share-setup" },
          task: true,
        },
        {
          id: "node",
          label: "Node & diagnostics",
          summary: nodeSummary(nodeStore.status.running, nodeStore.status.interfaces.length),
          icon: PhWrench,
          to: { name: "settings-node" },
        },
      ]),
    ];
  });
}
