# Client Implementation Set

This document is the reduced client-only implementation subset derived from `docs/plan/APIANnalysis.md`.
Only operations classified as `client` are included here. Grouping order is the recommended development sequence.

Total client operations: **104**

## Sequencing Rules
- Implement groups top to bottom unless a specific dependency changes.
- Do not include operations marked `server-only` or `unknown` unless the scope is explicitly widened.
- Use this file as the phase-by-phase build list for the message catalog and client bridge.

## 1. Core Discovery and Session
Build first. These operations establish connectivity, presence, hub identity, and basic dashboard state.

Operation count: **10**

| Function | Description |
|---|---|
| `GET /Client` | Lists all clients currently connected to the Hub, including their Reticulum identities and connection metadata |
| `GET /Events` | Return recent hub events. |
| `GET /Examples` | Return command descriptions and JSON payload examples. |
| `GET /Help` | Return the list of supported commands. |
| `POST /RCH` | Join an RCH instance as the supplied identity. |
| `PUT /RCH` | Leave an RCH instance as the supplied identity. |
| `POST /RTH` | Legacy compatibility alias for join. |
| `PUT /RTH` | Legacy compatibility alias for leave. |
| `GET /Status` | Return dashboard status metrics. |
| `GET /api/v1/app/info` | returns the configured application metadata and component versions |

## 2. Telemetry and Live Status
Add after core session flows so the client can display live system and telemetry updates.

Operation count: **3**

| Function | Description |
|---|---|
| `GET /Telemetry` | Retrieve telemetry snapshots since a timestamp. |
| `GET /events/system` | WebSocket stream for system status + events. |
| `GET /telemetry/stream` | WebSocket stream for live telemetry. |

## 3. Messaging and Chat
Deliver once session and live channels exist; this enables direct communication features.

Operation count: **2**

| Function | Description |
|---|---|
| `POST /Message` | Send a message into the hub. |
| `GET /messages/stream` | WebSocket stream for inbound/outbound messages. |

## 4. Topics and Distribution
Implement after messaging so topic-aware routing and subscriptions can layer on top.

Operation count: **3**

| Function | Description |
|---|---|
| `GET /Topic` | Retrieves a list of all Topic |
| `POST /Topic/Subscribe` | Subscribe a destination to a topic (Destination defaults to the authenticated identity when omitted). |
| `GET /Topic/{id}` | retrieve an existing Topic record based on the provided ID. |

## 5. Files and Media
Implement after messaging because attachments and media typically hang off message workflows.

Operation count: **8**

| Function | Description |
|---|---|
| `GET /File` | List stored file attachments. |
| `GET /File/{id}` | Retrieve a stored file by its ID. |
| `DELETE /File/{id}` | Delete a stored file by its ID. |
| `GET /File/{id}/raw` | Download a stored file by its ID (raw bytes). |
| `GET /Image` | List stored images. |
| `GET /Image/{id}` | Retrieve a stored image by its ID. |
| `DELETE /Image/{id}` | Delete a stored image by its ID. |
| `GET /Image/{id}/raw` | Download a stored image by its ID (raw bytes). |

## 6. Map, Markers, and Zones
Add once core data sync is stable; this unlocks operational map workflows.

Operation count: **8**

| Function | Description |
|---|---|
| `GET /api/markers` | List stored operator markers. |
| `POST /api/markers` | Create a new operator marker. |
| `GET /api/markers/symbols` | List available marker symbols. |
| `PATCH /api/markers/{object_destination_hash}/position` | Update marker coordinates. |
| `GET /api/zones` | List stored operational zones. |
| `POST /api/zones` | Create a new operational zone. |
| `PATCH /api/zones/{zone_id}` | Update zone metadata and/or polygon points. |
| `DELETE /api/zones/{zone_id}` | Delete an operational zone. |

## 7. R3AKT Mission Core
Start the R3AKT domain with missions, logs, changes, snapshots, and mission-level capability flows.

Operation count: **20**

| Function | Description |
|---|---|
| `GET /api/r3akt/capabilities/{identity}` | List capability grants for an identity. |
| `PUT /api/r3akt/capabilities/{identity}/{capability}` | Grant capability to identity. |
| `DELETE /api/r3akt/capabilities/{identity}/{capability}` | Revoke capability from identity. |
| `GET /api/r3akt/events` | List domain events. |
| `GET /api/r3akt/log-entries` | List mission log entries. |
| `POST /api/r3akt/log-entries` | Create or update mission log entry. |
| `GET /api/r3akt/mission-changes` | List mission changes. |
| `POST /api/r3akt/mission-changes` | Create or update mission change. |
| `GET /api/r3akt/missions` | List missions. |
| `POST /api/r3akt/missions` | Create or update mission. |
| `GET /api/r3akt/missions/{mission_uid}` | Retrieve mission by identifier. |
| `PATCH /api/r3akt/missions/{mission_uid}` | Patch mission fields. |
| `DELETE /api/r3akt/missions/{mission_uid}` | Soft-delete mission. |
| `PUT /api/r3akt/missions/{mission_uid}/parent` | Set or clear mission parent. |
| `GET /api/r3akt/missions/{mission_uid}/rde` | Retrieve mission role descriptor. |
| `PUT /api/r3akt/missions/{mission_uid}/rde` | Create or update mission role descriptor. |
| `GET /api/r3akt/missions/{mission_uid}/zones` | List mission zone identifiers. |
| `PUT /api/r3akt/missions/{mission_uid}/zones/{zone_id}` | Link mission to zone. |
| `DELETE /api/r3akt/missions/{mission_uid}/zones/{zone_id}` | Unlink mission from zone. |
| `GET /api/r3akt/snapshots` | List domain snapshots. |

## 8. R3AKT Teams, People, and Skills
Layer team structure after mission core is available.

Operation count: **20**

| Function | Description |
|---|---|
| `GET /api/r3akt/skills` | List skills. |
| `POST /api/r3akt/skills` | Create or update skill. |
| `GET /api/r3akt/task-skill-requirements` | List task skill requirements. |
| `POST /api/r3akt/task-skill-requirements` | Create or update task skill requirement. |
| `GET /api/r3akt/team-member-skills` | List team member skills. |
| `POST /api/r3akt/team-member-skills` | Create or update team member skill. |
| `GET /api/r3akt/team-members` | List team members. |
| `POST /api/r3akt/team-members` | Create or update team member. |
| `GET /api/r3akt/team-members/{team_member_uid}` | Retrieve a team member by uid. |
| `DELETE /api/r3akt/team-members/{team_member_uid}` | Delete a team member. |
| `GET /api/r3akt/team-members/{team_member_uid}/clients` | List linked client identities for a team member. |
| `PUT /api/r3akt/team-members/{team_member_uid}/clients/{client_identity}` | Link a team member to a client identity. |
| `DELETE /api/r3akt/team-members/{team_member_uid}/clients/{client_identity}` | Unlink a team member from a client identity. |
| `GET /api/r3akt/teams` | List teams. |
| `POST /api/r3akt/teams` | Create or update team. |
| `GET /api/r3akt/teams/{team_uid}` | Retrieve a team by uid. |
| `DELETE /api/r3akt/teams/{team_uid}` | Delete a team. |
| `GET /api/r3akt/teams/{team_uid}/missions` | List mission assignments for a team. |
| `PUT /api/r3akt/teams/{team_uid}/missions/{mission_uid}` | Link a team to a mission. |
| `DELETE /api/r3akt/teams/{team_uid}/missions/{mission_uid}` | Unlink a team from a mission. |

## 9. R3AKT Assets and Assignments
Build resource allocation after teams and missions are in place.

Operation count: **9**

| Function | Description |
|---|---|
| `GET /api/r3akt/assets` | List assets. |
| `POST /api/r3akt/assets` | Create or update asset. |
| `GET /api/r3akt/assets/{asset_uid}` | Retrieve an asset by uid. |
| `DELETE /api/r3akt/assets/{asset_uid}` | Delete an asset. |
| `GET /api/r3akt/assignments` | List mission task assignments. |
| `POST /api/r3akt/assignments` | Create or update assignment. |
| `PUT /api/r3akt/assignments/{assignment_uid}/assets` | Replace assignment asset links. |
| `PUT /api/r3akt/assignments/{assignment_uid}/assets/{asset_uid}` | Link assignment to asset. |
| `DELETE /api/r3akt/assignments/{assignment_uid}/assets/{asset_uid}` | Unlink assignment from asset. |

## 10. Checklists
Deliver last in phase 1 because checklist flows depend on mission, team, and assignment context.

Operation count: **21**

| Function | Description |
|---|---|
| `GET /checklists` | List active checklists. |
| `POST /checklists` | Create online checklist. |
| `POST /checklists/import/csv` | Import checklist from CSV. |
| `POST /checklists/offline` | Create offline checklist. |
| `GET /checklists/templates` | List checklist templates. |
| `POST /checklists/templates` | Create checklist template. |
| `GET /checklists/templates/{template_id}` | Get checklist template details. |
| `PATCH /checklists/templates/{template_id}` | Update checklist template. |
| `DELETE /checklists/templates/{template_id}` | Delete checklist template. |
| `POST /checklists/templates/{template_id}/clone` | Clone checklist template. |
| `GET /checklists/{checklist_id}` | Get checklist details. |
| `PATCH /checklists/{checklist_id}` | Update checklist metadata and mission link. |
| `DELETE /checklists/{checklist_id}` | Delete checklist instance. |
| `POST /checklists/{checklist_id}/feeds/{feed_id}` | Publish checklist to mission feed. |
| `POST /checklists/{checklist_id}/join` | Join checklist updates. |
| `POST /checklists/{checklist_id}/tasks` | Add checklist task row. |
| `DELETE /checklists/{checklist_id}/tasks/{task_id}` | Delete checklist task row. |
| `PATCH /checklists/{checklist_id}/tasks/{task_id}/cells/{column_id}` | Set checklist task cell value. |
| `PATCH /checklists/{checklist_id}/tasks/{task_id}/row-style` | Set checklist task row style. |
| `POST /checklists/{checklist_id}/tasks/{task_id}/status` | Set checklist task status. |
| `POST /checklists/{checklist_id}/upload` | Upload checklist and mark synced. |

