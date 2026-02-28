// Mirrors Reticulum-Telemetry-Hub reticulum_telemetry_hub.mission_domain.enums.TeamColor.
export const R3AKT_TEAM_COLORS = [
  "YELLOW",
  "RED",
  "BLUE",
  "ORANGE",
  "MAGENTA",
  "MAROON",
  "PURPLE",
  "DARK_BLUE",
  "CYAN",
  "TEAL",
  "GREEN",
  "DARK_GREEN",
  "BROWN",
] as const;

export type R3aktTeamColor = (typeof R3AKT_TEAM_COLORS)[number];

export const DEFAULT_R3AKT_TEAM_COLOR: R3aktTeamColor = "YELLOW";

const R3AKT_TEAM_COLOR_SET = new Set<string>(R3AKT_TEAM_COLORS);

export function normalizeR3aktTeamColor(
  value: unknown,
  fallback: R3aktTeamColor = DEFAULT_R3AKT_TEAM_COLOR,
): R3aktTeamColor {
  const normalized = String(value ?? "").trim().toUpperCase();
  if (R3AKT_TEAM_COLOR_SET.has(normalized)) {
    return normalized as R3aktTeamColor;
  }
  return fallback;
}

export function formatR3aktTeamColorLabel(value: R3aktTeamColor): string {
  return value
    .toLowerCase()
    .split("_")
    .map((segment) => `${segment.slice(0, 1).toUpperCase()}${segment.slice(1)}`)
    .join(" ");
}

export function formatR3aktTeamColor(value: unknown): string {
  return formatR3aktTeamColorLabel(normalizeR3aktTeamColor(value));
}
