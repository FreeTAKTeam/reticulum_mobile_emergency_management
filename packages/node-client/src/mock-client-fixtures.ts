import type { HubDirectoryPeerRecord, HubDirectoryUpdatedEvent } from "./contracts";
import { randomHex32 } from "./client-defaults";

export const MOCK_ANNOUNCED_PEERS = [
  "c3d4f7a6e01944ef8e620f5c5a146f1a",
  "4ecf4d0dcaf0f9126f493725314110bc",
  "e6dd8260de7cb8f3ff1f77a6810dcf9d",
  "99dd0a1cf3e95fc6f1d3a6765af96752",
  "a2f0d9a5fb6b94317802fca20af739b0",
];

export const MOCK_ANNOUNCED_IDENTITIES = MOCK_ANNOUNCED_PEERS.map(() => randomHex32());

const MOCK_HUB_PEERS: HubDirectoryPeerRecord[] = [
  {
    identity: randomHex32(),
    destinationHash: "7eb6e03ed67cd89bb3c5a7ac8713a109",
    displayName: "Pixel",
    announceCapabilities: ["r3akt", "emergencymessages", "telemetry"],
    clientType: "rem",
    registeredMode: "connected",
    lastSeen: "2026-04-02T12:43:28Z",
    status: "active",
  },
  {
    identity: randomHex32(),
    destinationHash: "c31298a1c68e30f7f3578fc03230591f",
    displayName: "Relay",
    announceCapabilities: ["r3akt", "emergencymessages", "telemetry_relay"],
    clientType: "rem",
    registeredMode: "connected",
    lastSeen: "2026-04-02T12:43:28Z",
    status: "active",
  },
  {
    identity: randomHex32(),
    destinationHash: "b07fd4a357fdb6b3500f5226346f56fd",
    displayName: "Console",
    announceCapabilities: ["r3akt", "group_chat"],
    clientType: "rem",
    registeredMode: "semi_autonomous",
    lastSeen: "2026-04-02T12:43:28Z",
    status: "active",
  },
];

export function mockHubDirectorySnapshot(): HubDirectoryUpdatedEvent {
  const yellowTeamUid = "d6b6e188b910d6bdd24d04b7a7ec5444";
  return {
    schemaVersion: 0,
    activeTeamUid: yellowTeamUid,
    effectiveConnectedMode: false,
    teams: [{ uid: yellowTeamUid, color: "YELLOW", teamName: "YELLOW" }],
    callerMemberships: [],
    members: MOCK_HUB_PEERS.map((peer) => ({
      ...peer,
      teamUid: yellowTeamUid,
      teamMemberUid: peer.identity,
    })),
    localTeams: [],
    items: MOCK_HUB_PEERS,
    receivedAtMs: Date.now(),
  };
}
