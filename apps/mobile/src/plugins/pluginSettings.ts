export type PluginSettingsFieldType = "boolean" | "number" | "select" | "text";

export type PluginSettingsValue = boolean | number | string;

export type PluginSettingsValues = Record<string, PluginSettingsValue>;

export interface PluginSettingsOption {
  label: string;
  value: string;
}

export interface PluginSettingsField {
  id: string;
  label: string;
  type: PluginSettingsFieldType;
  defaultValue: PluginSettingsValue;
  min?: number;
  max?: number;
  step?: number;
  placeholder?: string;
  options?: PluginSettingsOption[];
}

export interface PluginSettingsSection {
  pluginId: string;
  name: string;
  version: string;
  state: "Disabled" | "Enabled" | "Failed" | "Running" | "Stopped";
  description?: string;
  fields: PluginSettingsField[];
}

const PLUGIN_SETTINGS_STORAGE_PREFIX = "reticulum.mobile.pluginSettings.v1.";
const registeredPluginSettingsSections: PluginSettingsSection[] = [];

export function registerPluginSettingsSection(section: PluginSettingsSection): void {
  const existingIndex = registeredPluginSettingsSections.findIndex(
    (entry) => entry.pluginId === section.pluginId,
  );
  if (existingIndex >= 0) {
    registeredPluginSettingsSections.splice(existingIndex, 1, section);
    return;
  }
  registeredPluginSettingsSections.push(section);
}

export function listPluginSettingsSections(): PluginSettingsSection[] {
  return [...registeredPluginSettingsSections].sort((left, right) =>
    left.name.localeCompare(right.name),
  );
}

export function defaultPluginSettingsValues(section: PluginSettingsSection): PluginSettingsValues {
  return Object.fromEntries(
    section.fields.map((field) => [field.id, normalizePluginSettingsValue(field, undefined)]),
  );
}

export function loadPluginSettingsValues(section: PluginSettingsSection): PluginSettingsValues {
  const defaults = defaultPluginSettingsValues(section);
  try {
    const raw = localStorage.getItem(pluginSettingsStorageKey(section.pluginId));
    if (!raw) {
      return defaults;
    }
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    return Object.fromEntries(
      section.fields.map((field) => [
        field.id,
        normalizePluginSettingsValue(field, parsed[field.id]),
      ]),
    );
  } catch {
    return defaults;
  }
}

export function savePluginSettingsValues(
  pluginId: string,
  values: PluginSettingsValues,
): void {
  localStorage.setItem(pluginSettingsStorageKey(pluginId), JSON.stringify(values));
}

function pluginSettingsStorageKey(pluginId: string): string {
  return `${PLUGIN_SETTINGS_STORAGE_PREFIX}${pluginId}`;
}

function normalizePluginSettingsValue(
  field: PluginSettingsField,
  value: unknown,
): PluginSettingsValue {
  switch (field.type) {
    case "boolean":
      return typeof value === "boolean" ? value : Boolean(field.defaultValue);
    case "number": {
      const numeric = Number(value ?? field.defaultValue);
      if (!Number.isFinite(numeric)) {
        return Number(field.defaultValue);
      }
      const lowerBounded =
        typeof field.min === "number" ? Math.max(field.min, numeric) : numeric;
      return typeof field.max === "number" ? Math.min(field.max, lowerBounded) : lowerBounded;
    }
    case "select": {
      const selected = String(value ?? field.defaultValue);
      if (!field.options?.some((option) => option.value === selected)) {
        return String(field.defaultValue);
      }
      return selected;
    }
    case "text":
    default:
      return String(value ?? field.defaultValue);
  }
}
