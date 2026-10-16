export type SettingsSaveField =
  | "householdName"
  | "householdId"
  | "householdCounts"
  | "roleBadges"
  | "powerThresholdPercent";

export interface SettingsSaveFailure {
  message: string;
  field?: SettingsSaveField;
}

const FIELD_PATTERNS: Array<[RegExp, SettingsSaveField]> = [
  [/household name/i, "householdName"],
  [/household id/i, "householdId"],
  [/household adults|adults, children and pets/i, "householdCounts"],
  [/role badge/i, "roleBadges"],
  [/power saver threshold/i, "powerThresholdPercent"],
];

const GENERIC_INVALID_CONFIG_MESSAGE =
  "Settings not saved: the native runtime rejected the configuration (invalid config). "
  + "Check the household profile and power saver fields.";

function rawMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  if (error && typeof error === "object" && typeof (error as { message?: unknown }).message === "string") {
    return (error as { message: string }).message;
  }
  return String(error ?? "");
}

function errorCode(error: unknown): string {
  const code = (error as { code?: unknown } | null)?.code;
  return typeof code === "string" ? code : "";
}

/**
 * Presentation mapping for a failed Settings save: keeps the runtime's own reason
 * and points at the form field it names, so the banner never shows a bare
 * "invalid config".
 */
export function describeSettingsSaveError(error: unknown): SettingsSaveFailure {
  const message = rawMessage(error).trim();
  const field = FIELD_PATTERNS.find(([pattern]) => pattern.test(message))?.[1];
  if (field) {
    const sentence = message.replace(/\.?$/, ".");
    return { message: `Settings not saved: ${sentence}`, field };
  }
  if (errorCode(error) === "InvalidConfig" || /^invalid config$/i.test(message)) {
    return { message: GENERIC_INVALID_CONFIG_MESSAGE };
  }
  return { message: message || "Settings not saved." };
}
