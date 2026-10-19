import type { SosSettingsRecord } from "@reticulum/node-client";

import { normalizeReleaseSosSettings } from "../../stores/sosStore";

/** Draft for the SOS form, shared by Settings > SOS and the setup wizard. */
export function readSosDraft(settings: SosSettingsRecord): SosSettingsRecord {
  return { ...normalizeReleaseSosSettings(settings) };
}

/** The PIN has its own sheet; keep it out of the form's save cycle. */
export function sosSettingsWithoutPin(settings: SosSettingsRecord): SosSettingsRecord {
  const { deactivationPinHash: _hash, deactivationPinSalt: _salt, ...rest } = normalizeReleaseSosSettings(settings);
  return rest as SosSettingsRecord;
}

/** Form draft to save, keeping the PIN that is already stored. */
export function sosSettingsForSave(draft: SosSettingsRecord, stored: SosSettingsRecord): SosSettingsRecord {
  return {
    ...sosSettingsWithoutPin(draft),
    deactivationPinHash: stored.deactivationPinHash,
    deactivationPinSalt: stored.deactivationPinSalt,
  };
}
