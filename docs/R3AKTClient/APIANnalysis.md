# API Analysis

Source: `ReticulumCommunityHub-OAS.yaml`

Reduced actionable subset: [APIANnalysis_clientImplementationSet.md](APIANnalysis_clientImplementationSet.md)

Assessment labels:
- `client`: likely needed for client features.
- `server-only`: operational/admin/server-control oriented.
- `unknown`: ambiguous; needs product decision.

| Function | Description | Assessment |
|---|---|---|
| `GET /Client` | Lists all clients currently connected to the Hub, including their Reticulum identities and connection metadata | `client` |
| `POST /Client/{id}/Ban` | Ban an identity. | `server-only` |
| `POST /Client/{id}/Blackhole` | Blackhole an identity. | `server-only` |
| `POST /Client/{id}/Unban` | Remove ban/blackhole for an identity. | `server-only` |
| `GET /Command/DumpRouting` | Return connected destination hashes. | `server-only` |
| `POST /Command/FlushTelemetry` | Delete stored telemetry snapshots. | `server-only` |
| `POST /Command/ReloadConfig` | Reload config.ini from disk. | `server-only` |
| `GET /Config` | Return the raw config.ini content. | `server-only` |
| `PUT /Config` | Apply a new config.ini payload. | `server-only` |
| `POST /Config/Rollback` | Roll back config.ini using a backup. | `server-only` |
| `POST /Config/Validate` | Validate a config.ini payload without applying. | `server-only` |
| `POST /Control/Announce` | Send an immediate Reticulum announce. | `server-only` |
| `GET /Events` | Return recent hub events. | `client` |
| `GET /Examples` | Return command descriptions and JSON payload examples. | `server-only` |
| `GET /File` | List stored file attachments. | `client` |
| `GET /File/{id}` | Retrieve a stored file by its ID. | `client` |
| `DELETE /File/{id}` | Delete a stored file by its ID. | `client` |
| `GET /File/{id}/raw` | Download a stored file by its ID (raw bytes). | `client` |
| `GET /Help` | Return the list of supported commands. | `client` |
| `GET /Identities` | List identity moderation status entries. | `client` |
| `GET /Image` | List stored images. | `client` |
| `GET /Image/{id}` | Retrieve a stored image by its ID. | `client` |
| `DELETE /Image/{id}` | Delete a stored image by its ID. | `client` |
| `GET /Image/{id}/raw` | Download a stored image by its ID (raw bytes). | `client` |
| `POST /Message` | Send a message into the hub. | `client` |
| `POST /RCH` | Join an RCH instance as the supplied identity. | `client` |
| `PUT /RCH` | Leave an RCH instance as the supplied identity. | `client` |
| `POST /RTH` | Legacy compatibility alias for join. | `client` |
| `PUT /RTH` | Legacy compatibility alias for leave. | `client` |
| `GET /Reticulum/Config` | Return the raw Reticulum config content. | `client` |
| `PUT /Reticulum/Config` | Apply a new Reticulum config payload. | `client` |
| `POST /Reticulum/Config/Rollback` | Roll back Reticulum config using a backup. | `client` |
| `POST /Reticulum/Config/Validate` | Validate a Reticulum config payload without applying. | `client` |
| `GET /Reticulum/Discovery` | Return a live snapshot of Reticulum interface discovery state. | `client` |
| `GET /Reticulum/Interfaces/Capabilities` | Return Reticulum runtime interface capabilities. | `client` |
| `GET /Status` | Return dashboard status metrics. | `client` |
| `GET /Subscriber` | Retrieves a list of all Subscriber | `server-only` |
| `POST /Subscriber` | Creates a new Subscriber record. | `server-only` |
| `PATCH /Subscriber` | Updates an existing Subscriber record. | `server-only` |
| `DELETE /Subscriber` | Deletes an existing Subscriber record based on the provided ID. | `server-only` |
| `POST /Subscriber/Add` | Add a destination/topic subscriber mapping (admin). | `server-only` |
| `GET /Subscriber/{id}` | retrieve an existing Subscriber record based on the provided ID. | `server-only` |
| `GET /Telemetry` | Retrieve telemetry snapshots since a timestamp. | `client` |
| `GET /Topic` | Retrieves a list of all Topic | `client` |
| `POST /Topic` | Creates a new Topic record. | `server-only` |
| `PATCH /Topic` | Updates an existing Topic record. | `server-only` |
| `DELETE /Topic` | Deletes an existing Topic record based on the provided ID. | `server-only` |
| `POST /Topic/Associate` | Associate an attachment upload with a TopicID. | `client` |
| `POST /Topic/Subscribe` | Subscribe a destination to a topic (Destination defaults to the authenticated identity when omitted). | `client` |
| `GET /Topic/{id}` | retrieve an existing Topic record based on the provided ID. | `client` |
| `GET /api/markers` | List stored operator markers. | `client` |
| `POST /api/markers` | Create a new operator marker. | `client` |
| `GET /api/markers/symbols` | List available marker symbols. | `client` |
| `PATCH /api/markers/{object_destination_hash}/position` | Update marker coordinates. | `client` |
| `GET /api/r3akt/assets` | List assets. | `client` |
| `POST /api/r3akt/assets` | Create or update asset. | `client` |
| `GET /api/r3akt/assets/{asset_uid}` | Retrieve an asset by uid. | `client` |
| `DELETE /api/r3akt/assets/{asset_uid}` | Delete an asset. | `client` |
| `GET /api/r3akt/assignments` | List mission task assignments. | `client` |
| `POST /api/r3akt/assignments` | Create or update assignment. | `client` |
| `PUT /api/r3akt/assignments/{assignment_uid}/assets` | Replace assignment asset links. | `client` |
| `PUT /api/r3akt/assignments/{assignment_uid}/assets/{asset_uid}` | Link assignment to asset. | `client` |
| `DELETE /api/r3akt/assignments/{assignment_uid}/assets/{asset_uid}` | Unlink assignment from asset. | `client` |
| `GET /api/r3akt/capabilities/{identity}` | List capability grants for an identity. | `client` |
| `PUT /api/r3akt/capabilities/{identity}/{capability}` | Grant capability to identity. | `client` |
| `DELETE /api/r3akt/capabilities/{identity}/{capability}` | Revoke capability from identity. | `client` |
| `GET /api/r3akt/events` | List domain events. | `client` |
| `GET /api/r3akt/log-entries` | List mission log entries. | `client` |
| `POST /api/r3akt/log-entries` | Create or update mission log entry. | `client` |
| `GET /api/r3akt/mission-changes` | List mission changes. | `client` |
| `POST /api/r3akt/mission-changes` | Create or update mission change. | `client` |
| `GET /api/r3akt/missions` | List missions. | `client` |
| `POST /api/r3akt/missions` | Create or update mission. | `client` |
| `GET /api/r3akt/missions/{mission_uid}` | Retrieve mission by identifier. | `client` |
| `PATCH /api/r3akt/missions/{mission_uid}` | Patch mission fields. | `client` |
| `DELETE /api/r3akt/missions/{mission_uid}` | Soft-delete mission. | `client` |
| `PUT /api/r3akt/missions/{mission_uid}/parent` | Set or clear mission parent. | `client` |
| `GET /api/r3akt/missions/{mission_uid}/rde` | Retrieve mission role descriptor. | `client` |
| `PUT /api/r3akt/missions/{mission_uid}/rde` | Create or update mission role descriptor. | `client` |
| `GET /api/r3akt/missions/{mission_uid}/zones` | List mission zone identifiers. | `client` |
| `PUT /api/r3akt/missions/{mission_uid}/zones/{zone_id}` | Link mission to zone. | `client` |
| `DELETE /api/r3akt/missions/{mission_uid}/zones/{zone_id}` | Unlink mission from zone. | `client` |
| `GET /api/r3akt/skills` | List skills. | `client` |
| `POST /api/r3akt/skills` | Create or update skill. | `client` |
| `GET /api/r3akt/snapshots` | List domain snapshots. | `client` |
| `GET /api/r3akt/task-skill-requirements` | List task skill requirements. | `client` |
| `POST /api/r3akt/task-skill-requirements` | Create or update task skill requirement. | `client` |
| `GET /api/r3akt/team-member-skills` | List team member skills. | `client` |
| `POST /api/r3akt/team-member-skills` | Create or update team member skill. | `client` |
| `GET /api/r3akt/team-members` | List team members. | `client` |
| `POST /api/r3akt/team-members` | Create or update team member. | `client` |
| `GET /api/r3akt/team-members/{team_member_uid}` | Retrieve a team member by uid. | `client` |
| `DELETE /api/r3akt/team-members/{team_member_uid}` | Delete a team member. | `client` |
| `GET /api/r3akt/team-members/{team_member_uid}/clients` | List linked client identities for a team member. | `client` |
| `PUT /api/r3akt/team-members/{team_member_uid}/clients/{client_identity}` | Link a team member to a client identity. | `client` |
| `DELETE /api/r3akt/team-members/{team_member_uid}/clients/{client_identity}` | Unlink a team member from a client identity. | `client` |
| `GET /api/r3akt/teams` | List teams. | `client` |
| `POST /api/r3akt/teams` | Create or update team. | `client` |
| `GET /api/r3akt/teams/{team_uid}` | Retrieve a team by uid. | `client` |
| `DELETE /api/r3akt/teams/{team_uid}` | Delete a team. | `client` |
| `GET /api/r3akt/teams/{team_uid}/missions` | List mission assignments for a team. | `client` |
| `PUT /api/r3akt/teams/{team_uid}/missions/{mission_uid}` | Link a team to a mission. | `client` |
| `DELETE /api/r3akt/teams/{team_uid}/missions/{mission_uid}` | Unlink a team from a mission. | `client` |
| `GET /api/v1/app/info` | returns the configured application metadata and component versions | `client` |
| `GET /api/zones` | List stored operational zones. | `client` |
| `POST /api/zones` | Create a new operational zone. | `client` |
| `PATCH /api/zones/{zone_id}` | Update zone metadata and/or polygon points. | `client` |
| `DELETE /api/zones/{zone_id}` | Delete an operational zone. | `client` |
| `GET /checklists` | List active checklists. | `client` |
| `POST /checklists` | Create online checklist. | `client` |
| `POST /checklists/import/csv` | Import checklist from CSV. | `client` |
| `POST /checklists/offline` | Create offline checklist. | `client` |
| `GET /checklists/templates` | List checklist templates. | `client` |
| `POST /checklists/templates` | Create checklist template. | `client` |
| `GET /checklists/templates/{template_id}` | Get checklist template details. | `client` |
| `PATCH /checklists/templates/{template_id}` | Update checklist template. | `client` |
| `DELETE /checklists/templates/{template_id}` | Delete checklist template. | `client` |
| `POST /checklists/templates/{template_id}/clone` | Clone checklist template. | `client` |
| `GET /checklists/{checklist_id}` | Get checklist details. | `client` |
| `PATCH /checklists/{checklist_id}` | Update checklist metadata and mission link. | `client` |
| `DELETE /checklists/{checklist_id}` | Delete checklist instance. | `client` |
| `POST /checklists/{checklist_id}/feeds/{feed_id}` | Publish checklist to mission feed. | `client` |
| `POST /checklists/{checklist_id}/join` | Join checklist updates. | `client` |
| `POST /checklists/{checklist_id}/tasks` | Add checklist task row. | `client` |
| `DELETE /checklists/{checklist_id}/tasks/{task_id}` | Delete checklist task row. | `client` |
| `PATCH /checklists/{checklist_id}/tasks/{task_id}/cells/{column_id}` | Set checklist task cell value. | `client` |
| `PATCH /checklists/{checklist_id}/tasks/{task_id}/row-style` | Set checklist task row style. | `client` |
| `POST /checklists/{checklist_id}/tasks/{task_id}/status` | Set checklist task status. | `client` |
| `POST /checklists/{checklist_id}/upload` | Upload checklist and mark synced. | `client` |
| `GET /events/system` | WebSocket stream for system status + events. | `client` |
| `GET /messages/stream` | WebSocket stream for inbound/outbound messages. | `client` |
| `GET /telemetry/stream` | WebSocket stream for live telemetry. | `client` |
