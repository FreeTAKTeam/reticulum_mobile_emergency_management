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

/** One situation the operator chooses; the native runtime derives B04, MECP and EAM from it. */
export type HouseholdSituation =
  | "all_ok"
  | "sheltering_in_place"
  | "evacuating"
  | "someone_missing"
  | "lost"
  | "stranded"
  | "threat_nearby"
  | "need_supplies"
  | "medical_help_needed"
  | "sos";

export type HouseholdMode = "off" | "this_phone_reports" | "another_phone_reports";

export type SituationMessageKind = "community_status" | "event" | "eam" | "sos";

export type SituationMessageOutcome =
  | "queued"
  | "held_by_power_saver"
  | "skipped_unchanged"
  | "skipped_household_off"
  | "skipped_another_phone_reports"
  | "failed";

export interface SituationMessageRecord {
  kind: SituationMessageKind;
  outcome: SituationMessageOutcome;
  reference?: string;
}

export interface HouseholdReporterConflictRecord {
  sourceIdentity: string;
  householdName: string;
  updatedAtMs: number;
}

export interface HouseholdSituationRecord {
  situation?: HouseholdSituation;
  detail?: string;
  mecpCode?: string;
  eventUid?: string;
  sequence: number;
  updatedAtMs: number;
  householdMode: HouseholdMode;
  householdId?: string;
  situationEamFactors: string[];
  heldByPowerSaver: boolean;
  duplicateReporter?: HouseholdReporterConflictRecord;
  messages: SituationMessageRecord[];
}
