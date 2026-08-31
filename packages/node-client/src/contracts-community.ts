import type { HubMode } from "./contracts-core";

export type CircleTier = "inner" | "outer";
export type HouseholdStatus = "all_home" | "one_missing" | "evacuated" | "needs_help";
export type PreferredMapLayer = "base" | "satellite";

export interface CommunitySettingsRecord {
  householdId: string;
  householdName: string;
  adults: number;
  children: number;
  pets: number;
  roleBadges: string[];
  status: HouseholdStatus;
  preferredMapLayer: PreferredMapLayer;
}

export interface CommunityStatusProjectionRecord {
  householdId: string;
  householdName: string;
  adults: number;
  children: number;
  pets: number;
  roleBadges: string[];
  status: HouseholdStatus;
  saverActive: boolean;
  updatedAtMs: number;
  sourceIdentity: string;
}

export interface PowerPolicyRecord {
  enabled: boolean;
  thresholdPercent: 10 | 20 | 30;
}

export interface PowerStateRecord {
  batteryPercent?: number;
  charging: boolean;
  saverActive: boolean;
  updatedAtMs: number;
}

export interface BlockRadioSettings {
  region: string;
  profile: string;
  frequencyHz: number;
}

export interface BlockNetworkSettings {
  tcpClients: string[];
  broadcast: boolean;
  hubMode: HubMode;
  hubIdentityHash?: string;
  hubApiBaseUrl?: string;
  hubRefreshIntervalSeconds: number;
  radio?: BlockRadioSettings;
}

export interface BlockOnboardingDraft {
  network: BlockNetworkSettings;
  trustedDestinationHashes: string[];
  preferredMapLayer: PreferredMapLayer;
  expiresAtMs: number;
}

export interface SignedBlockOnboardingEnvelope { encodedText: string; }

export interface BlockOnboardingInspection {
  issuerPublicIdentityHex: string;
  issuerAppDestinationHex: string;
  issuerLxmfDestinationHex: string;
  signerFingerprint: string;
  issuedAtMs: number;
  expiresAtMs: number;
  network: BlockNetworkSettings;
  trustedDestinationHashes: string[];
  preferredMapLayer: PreferredMapLayer;
}

export interface BlockPeerTierRecord {
  destinationHex: string;
  circleTier: CircleTier;
}

export interface BlockOnboardingImportRequest {
  encodedText: string;
  confirmedSignerFingerprint: string;
  community: CommunitySettingsRecord;
  peerTiers: BlockPeerTierRecord[];
}

export interface BlockOnboardingImportResult {
  importedPeerCount: number;
  settingsUpdated: boolean;
}
