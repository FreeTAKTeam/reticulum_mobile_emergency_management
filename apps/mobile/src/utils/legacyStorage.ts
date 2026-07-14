export const LEGACY_SETTINGS_STORAGE_KEY = "reticulum.mobile.settings.v1";
export const LEGACY_SAVED_STORAGE_KEY = "reticulum.mobile.savedPeers.v1";
export const LEGACY_EAM_STORAGE_KEY = "reticulum.mobile.messages.v1";
export const LEGACY_EVENT_STORAGE_KEY = "reticulum.mobile.events.v1";
export const LEGACY_INBOX_STORAGE_KEY = "reticulum.mobile.inbox.v1";
export const LEGACY_TELEMETRY_STORAGE_KEY = "reticulum.mobile.telemetry.v1";
export const UI_SETTINGS_STORAGE_KEY = "reticulum.mobile.uiSettings.v1";

export type JsonRecord = Record<string, unknown>;

export function nowMs(): number {
  return Date.now();
}

export function readJson<T>(key: string): T | null {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) {
      return null;
    }
    return JSON.parse(raw) as T;
  } catch {
    return null;
  }
}

export function asRecord(value: unknown): JsonRecord | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as JsonRecord;
}

export function asTrimmedString(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

export function optionalNumber(value: unknown): number | undefined {
  if (value === undefined || value === null || value === "") {
    return undefined;
  }
  const numeric = Number(value);
  return Number.isFinite(numeric) ? numeric : undefined;
}
