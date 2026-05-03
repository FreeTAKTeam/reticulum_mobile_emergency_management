import { computed, reactive } from "vue";
import type { InstalledPluginRecord, PluginState } from "@reticulum/node-client";

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

export interface PluginSettingsAction {
  id: string;
  label: string;
  type: "send_lxmf";
  messageName: string;
  destinationField: string;
  bodyField: string;
  payloadFields: Record<string, string>;
}

export interface PluginSettingsSection {
  pluginId: string;
  name: string;
  version: string;
  state: PluginState;
  description?: string;
  fields: PluginSettingsField[];
  actions: PluginSettingsAction[];
}

const PLUGIN_SETTINGS_STORAGE_PREFIX = "reticulum.mobile.pluginSettings.v1.";
const registeredPluginSettingsSections = reactive<PluginSettingsSection[]>([]);
const installedPluginSettingsSections = reactive<PluginSettingsSection[]>([]);

export const pluginSettingsSections = computed<PluginSettingsSection[]>(() => {
  const merged = new Map<string, PluginSettingsSection>();
  for (const section of registeredPluginSettingsSections) {
    merged.set(section.pluginId, section);
  }
  for (const section of installedPluginSettingsSections) {
    merged.set(section.pluginId, section);
  }
  return [...merged.values()].sort((left, right) => left.name.localeCompare(right.name));
});

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

export function syncInstalledPluginSettingsSections(plugins: InstalledPluginRecord[]): void {
  installedPluginSettingsSections.splice(
    0,
    installedPluginSettingsSections.length,
    ...plugins.flatMap((plugin) => {
      const section = plugin.settings
        ? pluginSettingsSectionFromInstalledPlugin(plugin)
        : undefined;
      return section ? [section] : [];
    }),
  );
}

export function defaultPluginSettingsValues(section: PluginSettingsSection): PluginSettingsValues {
  return Object.fromEntries(
    section.fields.map((field) => [field.id, normalizePluginSettingsValue(field, undefined)]),
  );
}

function pluginSettingsSectionFromInstalledPlugin(
  plugin: InstalledPluginRecord,
): PluginSettingsSection | undefined {
  const schema = asRecord(plugin.settings?.schema);
  if (!schema) {
    return undefined;
  }
  const fields = fieldsFromSettingsSchema(schema);
  return {
    pluginId: plugin.id,
    name: plugin.name,
    version: plugin.version,
    state: plugin.state,
    description: asOptionalString(schema.description),
    fields,
    actions: actionsFromSettingsSchema(schema),
  };
}

function fieldsFromSettingsSchema(schema: Record<string, unknown>): PluginSettingsField[] {
  const explicitFields = Array.isArray(schema.fields)
    ? schema.fields
        .map((field) => fieldFromExplicitSchema(asRecord(field)))
        .filter((field): field is PluginSettingsField => Boolean(field))
    : [];
  if (explicitFields.length > 0) {
    return explicitFields;
  }

  const properties = asRecord(schema.properties);
  if (!properties) {
    return [];
  }
  return Object.entries(properties)
    .map(([id, raw]) => fieldFromJsonSchemaProperty(id, asRecord(raw)))
    .filter((field): field is PluginSettingsField => Boolean(field));
}

function fieldFromExplicitSchema(
  raw: Record<string, unknown> | undefined,
): PluginSettingsField | undefined {
  if (!raw) {
    return undefined;
  }
  const id = asOptionalString(raw.id);
  if (!id) {
    return undefined;
  }
  const type = normalizeFieldType(raw.type, raw);
  const options = optionsFromSchema(raw);
  return {
    id,
    label: asOptionalString(raw.label) ?? asOptionalString(raw.title) ?? id,
    type,
    defaultValue: defaultValueForField(type, raw, options),
    min: asOptionalNumber(raw.min) ?? asOptionalNumber(raw.minimum),
    max: asOptionalNumber(raw.max) ?? asOptionalNumber(raw.maximum),
    step: asOptionalNumber(raw.step),
    placeholder: asOptionalString(raw.placeholder),
    options,
  };
}

function actionsFromSettingsSchema(schema: Record<string, unknown>): PluginSettingsAction[] {
  if (!Array.isArray(schema.actions)) {
    return [];
  }
  return schema.actions
    .map((action) => actionFromSchema(asRecord(action)))
    .filter((action): action is PluginSettingsAction => Boolean(action));
}

function actionFromSchema(
  raw: Record<string, unknown> | undefined,
): PluginSettingsAction | undefined {
  if (!raw) {
    return undefined;
  }
  const id = asOptionalString(raw.id);
  const type = asOptionalString(raw.type);
  const messageName = asOptionalString(raw.messageName);
  const destinationField = asOptionalString(raw.destinationField);
  const bodyField = asOptionalString(raw.bodyField);
  if (
    !id
    || (type !== "send_lxmf" && type !== "sendPluginLxmf")
    || !messageName
    || !destinationField
    || !bodyField
  ) {
    return undefined;
  }
  return {
    id,
    label: asOptionalString(raw.label) ?? id,
    type: "send_lxmf",
    messageName,
    destinationField,
    bodyField,
    payloadFields: payloadFieldsFromSchema(raw.payloadFields),
  };
}

function payloadFieldsFromSchema(value: unknown): Record<string, string> {
  const raw = asRecord(value);
  if (!raw) {
    return {};
  }
  return Object.fromEntries(
    Object.entries(raw).flatMap(([payloadKey, fieldId]) => {
      if (typeof fieldId !== "string" || fieldId.trim().length === 0) {
        return [];
      }
      return [[payloadKey, fieldId]];
    }),
  );
}

function fieldFromJsonSchemaProperty(
  id: string,
  raw: Record<string, unknown> | undefined,
): PluginSettingsField | undefined {
  if (!raw || !id.trim()) {
    return undefined;
  }
  const type = normalizeFieldType(raw.type, raw);
  const options = optionsFromSchema(raw);
  return {
    id,
    label: asOptionalString(raw.title) ?? asOptionalString(raw.label) ?? id,
    type,
    defaultValue: defaultValueForField(type, raw, options),
    min: asOptionalNumber(raw.minimum) ?? asOptionalNumber(raw.min),
    max: asOptionalNumber(raw.maximum) ?? asOptionalNumber(raw.max),
    step: asOptionalNumber(raw.multipleOf) ?? asOptionalNumber(raw.step),
    placeholder: asOptionalString(raw.placeholder),
    options,
  };
}

function normalizeFieldType(
  rawType: unknown,
  schema: Record<string, unknown>,
): PluginSettingsFieldType {
  const type = String(rawType ?? "").toLowerCase();
  if (Array.isArray(schema.enum) || Array.isArray(schema.options) || type === "select") {
    return "select";
  }
  if (type === "boolean") {
    return "boolean";
  }
  if (type === "number" || type === "integer") {
    return "number";
  }
  return "text";
}

function optionsFromSchema(raw: Record<string, unknown>): PluginSettingsOption[] | undefined {
  if (Array.isArray(raw.options)) {
    const options = raw.options
      .map((option) => {
        if (typeof option === "string" || typeof option === "number" || typeof option === "boolean") {
          return {
            label: String(option),
            value: String(option),
          };
        }
        const record = asRecord(option);
        if (!record) {
          return undefined;
        }
        const value = asOptionValue(record.value);
        if (!value) {
          return undefined;
        }
        return {
          label: asOptionalString(record.label) ?? value,
          value,
        };
      })
      .filter((option): option is PluginSettingsOption => Boolean(option));
    return options.length > 0 ? options : undefined;
  }

  if (!Array.isArray(raw.enum)) {
    return undefined;
  }
  const enumNames = Array.isArray(raw.enumNames) ? raw.enumNames : [];
  const options = raw.enum.map((value, index) => ({
    label: asOptionalString(enumNames[index]) ?? String(value),
    value: String(value),
  }));
  return options.length > 0 ? options : undefined;
}

function defaultValueForField(
  type: PluginSettingsFieldType,
  raw: Record<string, unknown>,
  options?: PluginSettingsOption[],
): PluginSettingsValue {
  const explicitDefault = raw.defaultValue ?? raw.default;
  if (explicitDefault !== undefined) {
    switch (type) {
      case "boolean":
        return booleanSettingValue(explicitDefault);
      case "number":
        return Number.isFinite(Number(explicitDefault)) ? Number(explicitDefault) : 0;
      case "select":
        return String(explicitDefault);
      case "text":
      default:
        return String(explicitDefault);
    }
  }
  switch (type) {
    case "boolean":
      return false;
    case "number":
      return 0;
    case "select":
      return options?.[0]?.value ?? "";
    case "text":
    default:
      return "";
  }
}

function asRecord(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : undefined;
}

function asOptionalString(value: unknown): string | undefined {
  return typeof value === "string" && value.trim().length > 0 ? value : undefined;
}

function asOptionalNumber(value: unknown): number | undefined {
  const numeric = Number(value);
  return Number.isFinite(numeric) ? numeric : undefined;
}

function asOptionValue(value: unknown): string | undefined {
  if (typeof value === "string" && value.trim().length > 0) {
    return value;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return undefined;
}

function booleanSettingValue(value: unknown): boolean {
  if (typeof value === "boolean") {
    return value;
  }
  if (typeof value === "string") {
    const normalized = value.trim().toLowerCase();
    if (normalized === "true") {
      return true;
    }
    if (normalized === "false") {
      return false;
    }
  }
  return Boolean(value);
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
