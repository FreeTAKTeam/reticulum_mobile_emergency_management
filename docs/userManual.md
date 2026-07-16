# R.E.M. User Manual

R.E.M. stands for Reticulum Emergency Management.

It is a secure Mesh  coordination app for small teams.  It helps people share status, send chat messages, record events, manage checklists, and see recent positions over Reticulum mesh networking.

This manual uses plain language. It focuses on what each screen is for, what you can do there, and what to expect.

## Quick Start

- Open **Settings** and confirm your call sign.
- Open **Peers** and save the people you trust.
-Use **Chat** for direct messages.
- Use **Action Emergency Messages** to report team status.
- Use **Events** to add short timeline updates.
- Use **Checklists** to track shared tasks.
- Use **Map** to see positions when telemetry is enabled.
- Use **Dashboard** to see the overall picture.

## Main Navigation

The top of the app shows:

- the current page name
- saved peer and connected peer count
- whether REM is **Ready**

If the app says **Not Ready**, sending may be limited until the node starts.

The bottom bar gives fast access to:

- **Dashboard**
- **Chat**
- **Checklists**
- **Map**
- **More**

Use **More** to open:

- **Action Messages**
- **Events**
- **Peers**
- **Settings**

## Dashboard

![REM dashboard](screenshots/rem-dashboard.png)

The Dashboard gives a quick team picture.

Use it to see:

- readiness across security, capability, preparedness, medical, mobility, and communications
- checklist totals
- action message and event totals
- quick **Announce** and **Sync** controls

The status rings come from Action Messages. If there are no Action Messages yet, the rings stay at zero.

## Chat

![REM chat](screenshots/rem-chat.png)

Chat is for direct peer-to-peer messages.

Use it to:

- review conversations
- send a message to a selected peer
- read incoming LXMF messages
- open SOS positions on the Map when an emergency message includes location

If the page is empty, discover and save peers first, or wait for an incoming message.

## Action Messages

![REM action messages](screenshots/rem-action-messages.png)

Action Messages are structured status reports. They also feed the Dashboard.

Use them to tell the team:

- whether the area is secure
- whether the team has defensive capability
- whether supplies and power are adequate
- whether there is a medical problem
- whether people can move
- whether communications are working

The status colors are:

| Color | Plain meaning |
| --- | --- |
| Red | Immediate problem or serious limit |
| Yellow | Limited, degraded, or needs attention |
| Green | Working or acceptable |
| Unknown | unclear, Not confirmed yet |

Choose the lowest accurate color. Do not choose a better color just because conditions might improve later.

### Status Help

![REM status help](screenshots/rem-status-help.png)

The help page explains how to choose the Action Message colors. Use it when a team needs a shared rule for what Red, Yellow, Green, and Unknown mean.

## Events

![REM Events MECP composer](screenshots/rem-events-mecp.png)

Events are short timeline updates. Use them for things the team should know, such as:

- road blocked
- bridge out
- storm approaching
- need water
- stranded
- send rescue
- acknowledged
- drill message

REM Events now use **MECP**, the Mesh Emergency Communication Protocol. MECP is a short text code for emergency and everyday mesh messages. It is designed for low-bandwidth links and for people who may not share the same spoken language. See the MECP project here: [xiang-dev-1/MECP](https://github.com/xiang-dev-1/MECP).

You do not need to memorize MECP codes. REM lets you choose the plain-language options and shows the body before the event is added.

![REM MECP help](screenshots/rem-events-mecp-help.png)

The MECP Help screen explains the short message body, the urgency levels, and the event categories in one place.

### MECP Event Choices

| Part | What you choose | What REM sends |
| --- | --- | --- |
| Severity | Mayday, Urgent, Safety, or Routine | The number after `MECP/` |
| Category | Medical, Terrain, Weather, Supplies, Position, Coordination, Response, Drill, Life, Threat, Resources, or Beacon | The first letter of the event code |
| Event | A specific item, such as Injury, Road blocked, Need water, or Stranded | A short code like `M01`, `T01`, `S01`, or `P01` |
| Details | Optional extra words | Added after the code |
| Body preview | The exact message that will be shared | Example: `MECP/2/P01` |

### MECP Categories

| Code | Category | Used for |
| --- | --- | --- |
| M | Medical | Injuries, bleeding, burns, missing or found people |
| T | Terrain / Infrastructure | Roads, bridges, buildings, flooding, fire, hazards |
| W | Weather / Environment | Storms, visibility, cold, heat, air quality |
| S | Supplies | Water, food, medication, fuel, tools, power |
| P | Position / Movement | Stranded, evacuating, sheltering, en route, lost |
| C | Coordination | Send rescue, relay, confirm received, rendezvous |
| R | Response | Acknowledged, help coming, ETA, all clear |
| D | Drill / Test | Drill, test, end of drill, sent in error |
| L | Life / Leisure | Low-risk everyday messages that help people practice |
| X | Threat / Security | Unsafe areas, dangerous people, unrest, checkpoints |
| H | Have / Offer Resources | Available water, food, medical supplies, shelter, transport |
| B | Beacon | Distress beacon, beacon acknowledged, cancel beacon |

### How To Add An Event

1. Open **Events**.
2. Tap **+**.
3. Choose the severity.
4. Choose the category.
5. Choose the event.
6. Add optional details if needed.
7. Review the **Body** preview.
8. Tap **Add Event**.

Events require the node to be Ready before they can be sent.

## Checklists

![REM checklists](screenshots/rem-checklists.png)

Checklists are shared task lists.

Use them to:

- create a task list from a template
- upload a CSV checklist template
- track active, late, and completed tasks
- open a checklist and mark rows complete
- share task updates with peers

The checklist page shows task counts and a filter. If no checklist data is loaded yet, the page tells you that nothing is available.

### Creating A Checklist

1. Open **Checklists**.
2. Tap **+**.
3. Enter a title.
4. Add a subtitle or assignment label if useful.
5. Choose the checklist date and time.
6. (optional) Choose a template.
7. Tap **Create checklist**.

## Map

![REM map](screenshots/rem-map.png)

The Map shows recent peer positions when telemetry is available.

Use it to:

- see live or recent positions
- open emergency locations from SOS messages
- review position age
- tell whether a location is fresh or stale

If nobody has shared telemetry yet, the map may be empty.

## Peers

![REM peers](screenshots/rem-peers.png)

Peers are other REM-capable devices.

Use the Peers page to:

- see discovered REM clients
- save trusted peers
- connect or disconnect saved peers
- search peer names or destination text
- review hub directory entries when hub support is in use
- announce your presence

Discovery does not mean trust. Save only the peers you want to work with.

In **Semi-autonomous** RCH mode, the hub directory is authoritative for team
sharing. RCH returns only recent REM-capable identities belonging to teams you
share. If that directory cannot be refreshed or validated, REM shows an empty
hub directory and pauses team fanout instead of sending to locally discovered
peers. **Autonomous** mode continues to use locally managed peers, while
**Connected** mode sends through the selected hub.

### How Peer Sharing Works Today

When you save or select a peer, you are choosing who REM should try to share information with. This can include chat messages, Action Messages, Events, checklist updates, telemetry, and SOS updates, depending on what is enabled.

In the current version, peer sharing is not fully automatic in both directions. The other person also needs to save or select you on their device. If you select them but they do not select you, your device may try to share with them, but their device may not share the same kind of information back to you.

For a working team setup:

1. You save or select the other peer.
2. The other peer saves or selects you.
3. Both devices show Ready.
4. Send a small chat message or Event to confirm that sharing works.

Think of the peer list as your trusted working group. If someone should receive your operational updates, they should be in your peer list, and you should be in theirs.

## Settings

![REM settings](screenshots/rem-settings.png)

Settings is where you prepare the app before field use.

Use it to configure:

- call sign
- Reticulum network access
- announce behavior
- telemetry sharing
- checklist timing defaults
- RCH hub mode
- peer list import and export
- SOS emergency options
- node start, stop, and restart controls

## SOS Emergency

SOS is for urgent distress messages.

When enabled, it can:

- show a floating SOS button
- send the configured emergency message to saved peers
- include battery information
- include  position if a recent fix is available
- keep sending updates while the emergency is active

- require a PIN to cancel, if configured

Before relying on SOS, test it with trusted peers in a safe setting.

## Typical Field Workflow

1. Set your call sign in **Settings**.
2. Confirm networking and telemetry choices.
3. Save trusted devices in **Peers**.
4. Send or receive a chat message to confirm contact.
5. Create an Action Message for your current team status.
6. Watch the Dashboard for the group picture.
7. Add Events when something important changes.
8. Use Checklists for assigned work.
9. Use the Map when location sharing is part of the plan.

## Empty Screens

Some pages start empty on purpose.

| Screen | Why it may be empty |
| --- | --- |
| Dashboard | No Action Messages or activity yet |
| Chat | No conversations yet |
| Checklists | No checklist data or templates loaded yet |
| Map | No telemetry received yet |
| Events | No local or shared events yet |
| Peers | No REM announces heard yet |

An empty screen does not always mean something is wrong. It often means the app is waiting for local data, peer traffic, or a ready node.

## Glossary

| Term | Plain meaning |
| --- | --- |
| Action Message | A structured team status report. It feeds the Dashboard status rings. |
| Announce | A Mesh signal that tells REM devices that your device is online. |
| Call sign | The name your team sees for your device. |
| Checklist | A shared task list used to track work during an operation. |
| Connected peer | A saved peer that REM currently has an active connection with. |
| Dashboard | The screen that summarizes team status, checklist counts, and recent activity. |
| Destination | The Reticulum cryptographic address used to reach a device. |
| Event | A short timeline update about something relevant. |
| LXMF | The message protocol used by Reticulum for direct or delayed messages. |
| Map | The screen that shows recent positions when telemetry is available. |
| MECP | Mesh Emergency Communication Protocol. A short text format for clear emergency and field messages. |
| Node | The local Reticulum part of REM that sends, receives, announces, and connects. |
| Peer | Another REM-capable device or operator. |
| Ready | REM is prepared to send and receive through the local node. |
| RCH | Reticulum Community Hub. A hub service that can help with directory and connected-mode workflows. |
| Reticulum | The mesh networking system REM uses to communicate without normal internet protocol. |
| Saved peer | A peer you have chosen to trust and work with. |
| Selected peer | A peer you have chosen as a sharing target. In the current version, they also need to select you for two-way sharing. |
| SOS | An emergency mode for sending urgent distress information to trusted peers. |
| Telemetry | Position information, such as a recent location shown on the Map. |
