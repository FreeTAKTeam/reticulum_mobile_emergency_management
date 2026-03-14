import { expect, type Page } from "@playwright/test";

const STORAGE_KEYS = {
  messages: "reticulum.mobile.messages.v1",
  events: "reticulum.mobile.events.v1",
  telemetry: "reticulum.mobile.telemetry.v1",
  settings: "reticulum.mobile.settings.v1",
  savedPeers: "reticulum.mobile.savedPeers.v1",
} as const;

export interface ActionMessageSeed {
  callsign: string;
  groupName: string;
  securityStatus: "Red" | "Yellow" | "Green" | "Unknown";
  capabilityStatus: "Red" | "Yellow" | "Green" | "Unknown";
  preparednessStatus: "Red" | "Yellow" | "Green" | "Unknown";
  medicalStatus: "Red" | "Yellow" | "Green" | "Unknown";
  mobilityStatus: "Red" | "Yellow" | "Green" | "Unknown";
  commsStatus: "Red" | "Yellow" | "Green" | "Unknown";
  updatedAt: number;
  deletedAt?: number;
}

export interface EventSeed {
  command_id?: string;
  source?: {
    rns_identity?: string;
    display_name?: string;
  };
  timestamp?: string;
  command_type?: string;
  args?: {
    entry_uid?: string;
    mission_uid?: string;
    content?: string;
    callsign?: string;
    server_time?: string;
    client_time?: string;
    keywords?: string[];
    content_hashes?: string[];
    source_identity?: string;
    source_display_name?: string;
  };
  correlation_id?: string;
  topics?: string[];
  deleted_at?: number;
  uid?: string;
  entryUid?: string;
  missionUid?: string;
  callsign?: string;
  sourceIdentity?: string;
  sourceDisplayName?: string;
  type?: string;
  summary?: string;
  content?: string;
  updatedAt?: number;
  serverTime?: number;
  clientTime?: number;
  keywords?: string[];
  deletedAt?: number;
}

export interface TelemetrySeed {
  callsign: string;
  lat: number;
  lon: number;
  alt?: number;
  course?: number;
  speed?: number;
  accuracy?: number;
  updatedAt: number;
}

export interface SettingsSeed {
  displayName: string;
  clientMode: "auto" | "capacitor";
  autoConnectSaved: boolean;
  announceCapabilities: string;
  tcpClients: string[];
  broadcast: boolean;
  announceIntervalSeconds: number;
  showOnlyCapabilityVerified: boolean;
  telemetry: {
    enabled: boolean;
    publishIntervalSeconds: number;
    accuracyThresholdMeters?: number;
    staleAfterMinutes: number;
    expireAfterMinutes: number;
  };
  hub: {
    mode: "Disabled" | "RchLxmf" | "RchHttp";
    identityHash: string;
    apiBaseUrl: string;
    apiKey: string;
    refreshIntervalSeconds: number;
  };
}

export interface SavedPeerSeed {
  destination: string;
  label?: string;
  savedAt: number;
}

interface StorageSeed {
  messages?: ActionMessageSeed[];
  events?: EventSeed[];
  telemetry?: TelemetrySeed[];
  settings?: SettingsSeed;
  savedPeers?: SavedPeerSeed[];
}

export const defaultSettings: SettingsSeed = {
  displayName: "emergency-ops-mobile",
  clientMode: "auto",
  autoConnectSaved: true,
  announceCapabilities: "R3AKT,EMergencyMessages",
  tcpClients: ["rmap.world:4242"],
  broadcast: true,
  announceIntervalSeconds: 1800,
  showOnlyCapabilityVerified: true,
  telemetry: {
    enabled: false,
    publishIntervalSeconds: 10,
    staleAfterMinutes: 30,
    expireAfterMinutes: 180,
  },
  hub: {
    mode: "Disabled",
    identityHash: "",
    apiBaseUrl: "",
    apiKey: "",
    refreshIntervalSeconds: 3600,
  },
};

export async function seedAppStorage(page: Page, seed: StorageSeed = {}): Promise<void> {
  await page.addInitScript(
    ({ keys, payload }) => {
      window.localStorage.clear();

      if (payload.messages) {
        window.localStorage.setItem(keys.messages, JSON.stringify(payload.messages));
      }

      if (payload.events) {
        window.localStorage.setItem(keys.events, JSON.stringify(payload.events));
      }

      if (payload.telemetry) {
        window.localStorage.setItem(keys.telemetry, JSON.stringify(payload.telemetry));
      }

      if (payload.settings) {
        window.localStorage.setItem(keys.settings, JSON.stringify(payload.settings));
      }

      if (payload.savedPeers) {
        window.localStorage.setItem(keys.savedPeers, JSON.stringify(payload.savedPeers));
      }
    },
    {
      keys: STORAGE_KEYS,
      payload: {
        messages: seed.messages,
        events: seed.events,
        telemetry: seed.telemetry,
        settings: seed.settings,
        savedPeers: seed.savedPeers,
      },
    },
  );
}

export async function gotoApp(page: Page, path: string): Promise<void> {
  await page.goto(path);
  await expect(page.getByText("Emergency Ops", { exact: true })).toBeVisible();
}
