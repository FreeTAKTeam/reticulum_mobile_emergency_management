import { CANONICAL_TEAM_UIDS, YELLOW_TEAM_UID } from "@reticulum/node-client";

import type { NodeUiSettings } from "../types/domain";

type TeamPreferences = NodeUiSettings["teams"];

function stringValue(value: unknown): string {
  return typeof value === "string" ? value : "";
}

export function normalizeTeamPreferences(
  value: Partial<TeamPreferences> | null | undefined,
  fallback: TeamPreferences = {
    activeTeamUid: YELLOW_TEAM_UID,
    aliases: [],
    localTeams: [],
    localTeamsInitialized: false,
  },
): TeamPreferences {
  const requestedTeamUid = stringValue(value?.activeTeamUid).trim().toLowerCase();
  const fallbackTeamUid = stringValue(fallback.activeTeamUid).trim().toLowerCase();
  const activeTeamUid = CANONICAL_TEAM_UIDS.has(requestedTeamUid)
    ? requestedTeamUid
    : CANONICAL_TEAM_UIDS.has(fallbackTeamUid) ? fallbackTeamUid : YELLOW_TEAM_UID;
  const aliases = Array.isArray(value?.aliases)
    ? value.aliases
      .map((entry) => ({
        teamUid: stringValue(entry?.teamUid).trim().toLowerCase(),
        alias: stringValue(entry?.alias).trim().slice(0, 48),
      }))
      .filter((entry) => CANONICAL_TEAM_UIDS.has(entry.teamUid) && entry.alias.length > 0)
      .filter((entry, index, all) => (
        all.findIndex((candidate) => candidate.teamUid === entry.teamUid) === index
      ))
      .slice(0, 13)
    : fallback.aliases.map((entry) => ({ ...entry }));
  const localTeamsInitialized = value?.localTeamsInitialized === true
    || (value?.localTeamsInitialized === undefined && fallback.localTeamsInitialized);
  const localTeams = (Array.isArray(value?.localTeams) ? value.localTeams : fallback.localTeams)
    .map((team) => ({
      teamUid: stringValue(team?.teamUid).trim().toLowerCase(),
      memberDestinations: Array.isArray(team?.memberDestinations)
        ? team.memberDestinations
          .map((destination) => stringValue(destination).trim().toLowerCase())
          .filter((destination) => /^[0-9a-f]{32}$/.test(destination))
          .filter((destination, index, all) => all.indexOf(destination) === index)
        : [],
    }))
    .filter((team) => CANONICAL_TEAM_UIDS.has(team.teamUid))
    .filter((team, index, all) => (
      all.findIndex((candidate) => candidate.teamUid === team.teamUid) === index
    ))
    .slice(0, 13);
  if (localTeamsInitialized && !localTeams.some((team) => team.teamUid === YELLOW_TEAM_UID)) {
    localTeams.unshift({ teamUid: YELLOW_TEAM_UID, memberDestinations: [] });
  }
  return { activeTeamUid, aliases, localTeams, localTeamsInitialized };
}
