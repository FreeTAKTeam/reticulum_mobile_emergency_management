import { describe, expect, it } from "vitest";

import { YELLOW_TEAM_UID } from "./contracts";
import { toHubDirectoryUpdatedEvent } from "./hub-directory-converter";

const BLUE_TEAM_UID = "43341e5c822d99857fa6e8641f2ca9c0";

describe("hub TEAM directory conversion", () => {
  it("maps legacy flat peers to Yellow without losing compatibility items", () => {
    const snapshot = toHubDirectoryUpdatedEvent({
      effective_connected_mode: false,
      items: [{
        identity: "11".repeat(16),
        destination_hash: "22".repeat(16),
        announce_capabilities: ["r3akt", "emergencymessages"],
      }],
      received_at_ms: 123,
    });

    expect(snapshot.schemaVersion).toBe(0);
    expect(snapshot.activeTeamUid).toBe(YELLOW_TEAM_UID);
    expect(snapshot.items).toHaveLength(1);
    expect(snapshot.members).toMatchObject([{
      teamUid: YELLOW_TEAM_UID,
      teamMemberUid: "11".repeat(16),
      destinationHash: "22".repeat(16),
    }]);
  });

  it("preserves overlapping canonical memberships and ignores custom teams", () => {
    const destination = "33".repeat(16);
    const member = {
      team_member_uid: "member-1",
      identity: "44".repeat(16),
      destination_hash: destination,
      announce_capabilities: ["r3akt", "emergencymessages"],
    };
    const snapshot = toHubDirectoryUpdatedEvent({
      schema_version: 2,
      active_team_uid: BLUE_TEAM_UID,
      teams: [
        { uid: YELLOW_TEAM_UID, color: "YELLOW", team_name: "Yellow" },
        { uid: BLUE_TEAM_UID, color: "BLUE", team_name: "Blue" },
        { uid: "custom-team", color: "BLACK", team_name: "Custom" },
      ],
      caller_memberships: [
        { team_uid: BLUE_TEAM_UID, team_member_uid: "caller-blue" },
        { team_uid: "custom-team", team_member_uid: "caller-custom" },
      ],
      members: [
        { ...member, team_uid: YELLOW_TEAM_UID },
        { ...member, team_uid: BLUE_TEAM_UID },
        { ...member, team_uid: "custom-team" },
      ],
      local_teams: [{
        team_uid: BLUE_TEAM_UID,
        member_destinations: [destination],
      }],
      items: [],
    });

    expect(snapshot.activeTeamUid).toBe(BLUE_TEAM_UID);
    expect(snapshot.teams.map((team) => team.uid)).toEqual([YELLOW_TEAM_UID, BLUE_TEAM_UID]);
    expect(snapshot.callerMemberships).toEqual([{
      teamUid: BLUE_TEAM_UID,
      teamMemberUid: "caller-blue",
    }]);
    expect(snapshot.members).toHaveLength(2);
    expect(snapshot.localTeams).toEqual([{
      teamUid: BLUE_TEAM_UID,
      memberDestinations: [destination],
    }]);
    expect(snapshot.members.map((entry) => entry.destinationHash)).toEqual([
      destination,
      destination,
    ]);
  });
});
