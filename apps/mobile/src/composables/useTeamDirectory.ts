import { computed } from "vue";
import { CANONICAL_TEAMS, YELLOW_TEAM_UID } from "@reticulum/node-client";

import { useNodeStore } from "../stores/nodeStore";
import type {
  HubTeamMemberRecord,
  HubTeamRecord,
  LocalTeamRecord,
} from "../types/domain";
import { encodeLocalTeamExchange, parseLocalTeamExchange } from "../utils/localTeamExchange";

export interface TeamPeerRow {
  destination: string;
  displayName: string;
  localSaved: boolean;
  localMember: boolean;
  member?: HubTeamMemberRecord;
}

export interface TeamDirectorySection {
  team: HubTeamRecord;
  label: string;
  rows: TeamPeerRow[];
  total: number;
  connected: number;
  reachable: number;
  active: boolean;
  local: boolean;
  rch: boolean;
}

const TEAM_COLORS: Record<string, string> = {
  YELLOW: "#ffd22e",
  RED: "#ff4d57",
  BLUE: "#39a8ff",
  ORANGE: "#ff9d38",
  MAGENTA: "#ee65c5",
  MAROON: "#a95367",
  PURPLE: "#9d72ff",
  DARK_BLUE: "#4576c9",
  CYAN: "#37d5e8",
  TEAL: "#3bc4af",
  GREEN: "#55d879",
  DARK_GREEN: "#3a9c5a",
  BROWN: "#a8794f",
};

export function teamColorHex(color: string): string {
  return TEAM_COLORS[color.toUpperCase()] ?? "#7fcfff";
}

export function teamColorLabel(color: string): string {
  return color
    .toLowerCase()
    .replaceAll("_", " ")
    .replace(/\b\w/g, (character) => character.toUpperCase());
}

export function useTeamDirectory() {
  const nodeStore = useNodeStore();

  const localTeams = computed<LocalTeamRecord[]>(() => (
    nodeStore.settings.teams.localTeamsInitialized
      ? nodeStore.settings.teams.localTeams
      : [{
          teamUid: YELLOW_TEAM_UID,
          memberDestinations: nodeStore.savedPeers.map((peer) => peer.destination.toLowerCase()),
        }]
  ));

  const activeTeamUid = computed(
    () => nodeStore.hubDirectorySnapshot?.activeTeamUid
      || nodeStore.settings.teams.activeTeamUid
      || YELLOW_TEAM_UID,
  );
  const reachableDestinations = computed(
    () => new Set(nodeStore.reachablePeers.map((peer) => peer.destination.toLowerCase())),
  );
  const connectedDestinations = computed(
    () => new Set(nodeStore.connectedPeers.map((peer) => peer.destination.toLowerCase())),
  );

  function localAlias(teamUid: string): string {
    return nodeStore.settings.teams.aliases
      .find((entry) => entry.teamUid === teamUid)?.alias ?? "";
  }

  function canonicalTeam(teamUid: string): HubTeamRecord {
    const canonical = CANONICAL_TEAMS.find((team) => team.uid === teamUid);
    return nodeStore.hubDirectorySnapshot?.teams.find((team) => team.uid === teamUid) ?? {
      uid: teamUid,
      color: canonical?.color ?? "TEAM",
      teamName: teamColorLabel(canonical?.color ?? "Team"),
    };
  }

  function teamLabel(teamUid: string): string {
    const team = canonicalTeam(teamUid);
    const fallback = team.teamName && team.teamName !== team.color
      ? team.teamName
      : teamColorLabel(team.color);
    return localAlias(teamUid) || fallback;
  }

  const selectableTeams = computed(() => {
    const memberships = new Set(
      (nodeStore.hubDirectorySnapshot?.callerMemberships ?? []).map(({ teamUid }) => teamUid),
    );
    for (const team of localTeams.value) memberships.add(team.teamUid);
    memberships.add(YELLOW_TEAM_UID);
    return [...memberships].map(canonicalTeam).sort((left, right) => {
      if (left.uid === YELLOW_TEAM_UID) return -1;
      if (right.uid === YELLOW_TEAM_UID) return 1;
      return teamLabel(left.uid).localeCompare(teamLabel(right.uid));
    });
  });

  const availableLocalTeams = computed(() => {
    const existing = new Set(localTeams.value.map((team) => team.teamUid));
    return CANONICAL_TEAMS.filter((team) => !existing.has(team.uid));
  });

  function rowsFor(teamUid: string): TeamPeerRow[] {
    const rows = new Map<string, TeamPeerRow>();
    const localDestinations = new Set(
      localTeams.value.find((team) => team.teamUid === teamUid)?.memberDestinations ?? [],
    );
    for (const peer of nodeStore.savedPeers) {
      const key = peer.destination.toLowerCase();
      if (!localDestinations.has(key)) continue;
      rows.set(key, {
        destination: peer.destination,
        displayName: nodeStore.discoveredByDestination[peer.destination]?.announcedName
          || peer.label
          || peer.displayName
          || "No label",
        localSaved: true,
        localMember: true,
      });
    }
    for (const member of nodeStore.hubDirectorySnapshot?.members ?? []) {
      if (member.teamUid !== teamUid || !member.destinationHash) continue;
      const key = member.destinationHash.toLowerCase();
      const current = rows.get(key);
      rows.set(key, {
        destination: member.destinationHash,
        displayName: current?.displayName || member.displayName || member.identity,
        localSaved: current?.localSaved ?? false,
        localMember: current?.localMember ?? false,
        member,
      });
    }
    return [...rows.values()].sort((left, right) => left.displayName.localeCompare(right.displayName));
  }

  const teamSections = computed<TeamDirectorySection[]>(() => selectableTeams.value.map((team) => {
    const rows = rowsFor(team.uid);
    return {
      team,
      label: teamLabel(team.uid),
      rows,
      total: rows.length,
      connected: rows.filter((row) => (
        connectedDestinations.value.has(row.destination.toLowerCase())
      )).length,
      reachable: rows.filter((row) => (
        reachableDestinations.value.has(row.destination.toLowerCase())
      )).length,
      active: team.uid === activeTeamUid.value,
      local: localTeams.value.some((localTeam) => localTeam.teamUid === team.uid),
      rch: (nodeStore.hubDirectorySnapshot?.callerMemberships ?? [])
        .some((membership) => membership.teamUid === team.uid),
    };
  }));

  const activeSection = computed(() => (
    teamSections.value.find((section) => section.active)
      ?? teamSections.value.find((section) => section.team.uid === YELLOW_TEAM_UID)
  ));

  function settingsUpdate(options: {
    activeTeamUid?: string;
    aliases?: typeof nodeStore.settings.teams.aliases;
    localTeams?: LocalTeamRecord[];
  } = {}) {
    return {
      activeTeamUid: options.activeTeamUid ?? activeTeamUid.value,
      aliases: (options.aliases ?? nodeStore.settings.teams.aliases).map((entry) => ({ ...entry })),
      localTeams: (options.localTeams ?? localTeams.value).map((team) => ({
        ...team,
        memberDestinations: [...team.memberDestinations],
      })),
      localTeamsInitialized: true,
    };
  }

  async function setActiveTeam(teamUid: string): Promise<void> {
    if (!selectableTeams.value.some((team) => team.uid === teamUid)) {
      throw new Error("Select an available team.");
    }
    await nodeStore.setActiveTeam(teamUid);
    await nodeStore.updateSettings({
      teams: settingsUpdate({ activeTeamUid: teamUid }),
    });
  }

  async function saveTeamAlias(teamUid: string, value: string): Promise<string> {
    const alias = value.trim().slice(0, 48);
    const aliases = nodeStore.settings.teams.aliases.filter((entry) => entry.teamUid !== teamUid);
    if (alias) aliases.push({ teamUid, alias });
    await nodeStore.updateSettings({ teams: settingsUpdate({ aliases }) });
    return alias;
  }

  async function createLocalTeam(teamUid: string, value: string): Promise<void> {
    if (!availableLocalTeams.value.some((team) => team.uid === teamUid)) {
      throw new Error("Select an available team color.");
    }
    const nextTeams = [
      ...localTeams.value.map((team) => ({
        ...team,
        memberDestinations: [...team.memberDestinations],
      })),
      { teamUid, memberDestinations: [] },
    ];
    const alias = value.trim().slice(0, 48);
    const aliases = nodeStore.settings.teams.aliases.filter((entry) => entry.teamUid !== teamUid);
    if (alias) aliases.push({ teamUid, alias });
    await nodeStore.updateSettings({
      teams: settingsUpdate({ aliases, localTeams: nextTeams }),
    });
  }

  async function deleteLocalTeam(teamUid: string): Promise<void> {
    if (teamUid === YELLOW_TEAM_UID) {
      throw new Error("Yellow is the default team and cannot be deleted.");
    }
    const nextTeams = localTeams.value.filter((team) => team.teamUid !== teamUid);
    const aliases = nodeStore.settings.teams.aliases.filter((entry) => entry.teamUid !== teamUid);
    const nextActiveTeamUid = activeTeamUid.value === teamUid ? YELLOW_TEAM_UID : activeTeamUid.value;
    if (activeTeamUid.value === teamUid) await nodeStore.setActiveTeam(YELLOW_TEAM_UID);
    await nodeStore.updateSettings({
      teams: settingsUpdate({
        activeTeamUid: nextActiveTeamUid,
        aliases,
        localTeams: nextTeams,
      }),
    });
  }

  async function addLocalMember(teamUid: string, destination: string): Promise<void> {
    const normalizedDestination = destination.trim().toLowerCase();
    if (!normalizedDestination) throw new Error("Select a saved peer.");
    const nextTeams = localTeams.value.map((team) => team.teamUid === teamUid
      ? {
          ...team,
          memberDestinations: [...new Set([...team.memberDestinations, normalizedDestination])],
        }
      : { ...team, memberDestinations: [...team.memberDestinations] });
    await nodeStore.updateSettings({ teams: settingsUpdate({ localTeams: nextTeams }) });
  }

  async function removeLocalMember(teamUid: string, destination: string): Promise<void> {
    const normalizedDestination = destination.toLowerCase();
    const nextTeams = localTeams.value.map((team) => team.teamUid === teamUid
      ? {
          ...team,
          memberDestinations: team.memberDestinations.filter(
            (item) => item !== normalizedDestination,
          ),
        }
      : { ...team, memberDestinations: [...team.memberDestinations] });
    await nodeStore.updateSettings({ teams: settingsUpdate({ localTeams: nextTeams }) });
  }

  function addablePeers(teamUid: string) {
    const members = new Set(
      localTeams.value.find((team) => team.teamUid === teamUid)?.memberDestinations ?? [],
    );
    return nodeStore.savedPeers.filter((peer) => !members.has(peer.destination.toLowerCase()));
  }

  function exportLocalTeamText(teamUid: string): string {
    const team = localTeams.value.find((item) => item.teamUid === teamUid);
    if (!team) throw new Error("Only local teams can be exported.");
    return encodeLocalTeamExchange(
      teamUid,
      canonicalTeam(teamUid).color,
      team.memberDestinations.map((destination) => ({
        destination,
        label: nodeStore.savedByDestination[destination]?.label,
      })),
    );
  }

  async function importLocalTeamPayload(payload: string): Promise<string> {
    const imported = parseLocalTeamExchange(payload);
    const previouslySaved = new Set(Object.keys(nodeStore.savedByDestination));
    for (const member of imported.members) {
      if (!previouslySaved.has(member.destination)) await nodeStore.savePeer(member.destination);
      if (member.label) await nodeStore.setPeerLabel(member.destination, member.label);
    }
    const importedDestinations = new Set(imported.members.map(({ destination }) => destination));
    const records = localTeams.value.map((team) => ({
      ...team,
      memberDestinations: team.teamUid === imported.teamUid
        ? [...new Set([...team.memberDestinations, ...importedDestinations])]
        : team.teamUid === YELLOW_TEAM_UID && imported.teamUid !== YELLOW_TEAM_UID
          ? team.memberDestinations.filter((destination) => (
              previouslySaved.has(destination) || !importedDestinations.has(destination)
            ))
          : [...team.memberDestinations],
    }));
    if (!records.some(({ teamUid }) => teamUid === imported.teamUid)) {
      records.push({ teamUid: imported.teamUid, memberDestinations: [...importedDestinations] });
    }
    await nodeStore.updateSettings({ teams: settingsUpdate({ localTeams: records }) });
    return imported.teamUid;
  }

  function connectionStatus(destination: string): "Connected" | "Reachable" | "Offline" {
    const key = destination.toLowerCase();
    if (connectedDestinations.value.has(key)) return "Connected";
    return reachableDestinations.value.has(key) ? "Reachable" : "Offline";
  }

  return {
    activeSection,
    activeTeamUid,
    addablePeers,
    addLocalMember,
    availableLocalTeams,
    canonicalTeam,
    connectedDestinations,
    connectionStatus,
    createLocalTeam,
    deleteLocalTeam,
    exportLocalTeamText,
    importLocalTeamPayload,
    localAlias,
    localTeams,
    reachableDestinations,
    removeLocalMember,
    rowsFor,
    saveTeamAlias,
    selectableTeams,
    setActiveTeam,
    teamLabel,
    teamSections,
  };
}
