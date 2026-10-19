# Household situation: choose once, send everything

Status: phase 1 (runtime command, table and household mode) implemented; phases 2 and 3 open. Written 2026-10-19 against `main` at 5a9196e
plus the uncommitted field fixes on `corvo/field-ui-fixes` (SOS B01/B03 events,
decoded B04 cards, compact household card).

## 1. The problem

A REM device can report "how are we doing" in three unrelated ways today. Peers see
them in different places, with different audiences and rules.

| Signal | What it carries | Who receives it | Where it shows | Saver mode | Freshness |
|---|---|---|---|---|---|
| **EAM** (Emergency Action Message) | Six factors, each Red / Yellow / Green / Unknown: security, capability, preparedness, medical, mobility, comms | Active team roster | EAM screen; dashboard Team Status rings | Denied | `ttl_seconds`, else latest wins |
| **B04 household beacon** | Household profile and one of `all_home`, `one_missing`, `evacuated`, `needs_help` | All saved peers, Inner and Outer Circle | Dashboard household card; Peers roster; **also** the Events timeline | Allowed (one transition) | 7 days |
| **MECP events** | Incident codes. Several describe the same situations: `P02` evacuating, `P03` sheltering, `P06` lost, `P07` group separated, `M*` medical, `S*` needs, `C01` send rescue, `R07` all clear, `B01`/`B03` SOS beacon | Team event replication | Events timeline | Denied (except SOS B01/B03) | None |

What this causes in the field:

1. **The same fact has to be entered several times.** A household whose child is
   missing should tap *Someone Missing* (B04), set Mobility to Red (EAM) and post
   `P07` (MECP). In practice people do one of the three.
2. **Peers see contradictions.** "All Home" on the household card next to a Red
   mobility ring and a `P07` event, with nothing linking them.
3. **Mixed presentation.** B04 is a beacon but is listed as an incident in Events.
   EAM never appears there.

## 2. The idea

The operator picks **one situation**. The Rust runtime works out every message that
situation implies and sends each one through its existing contract, to its existing
audience. Nobody fills in factors or picks MECP codes by hand for this.

```
 operator taps            runtime derives and sends (one command, one place)
 ───────────────          ─────────────────────────────────────────────────────
 "Someone missing"  ──►   B04  status = one_missing           → all saved peers
                          MECP event  MECP/1/P07 #HH_…        → team
                          EAM  security = Red (others kept)    → team
```

Because every message comes from the same choice, receivers never see the three
disagree. No aggregation or reconciliation is needed on the receiving side.

### The constraint we keep

The P0 community plan
([`docs/goals/p0-community-foundation/PLAN.md`](../goals/p0-community-foundation/PLAN.md))
forbids *"EAM masquerading as MECP household status"*. This design keeps that: the
EAM sent is a real EAM in the EAM contract, the event is a real MECP event, and B04
stays `REMCS1` v1. They are three honest messages produced from one decision, not one
message pretending to be another. Wire formats and delivery scopes do not change.

## 3. Situations

One list, shown as the single choice on the dashboard card. Each row is the complete
fan-out for that situation. "B04" applies only on the phone that reports for the
household (see 3.5).

EAM factors keep their meaning. **Mobility** is the *ability* to move, not the fact of
moving, so only *Stranded* touches it. **Security** carries how safe the situation
is.

| Situation (what the operator taps) | B04 status | MECP event (team) | EAM factors set (team) | Other |
|---|---|---|---|---|
| **All OK** | `all_home` | `MECP/2/R07` *Situation resolved / all clear*, always | Restores the factors the previous situation set (see 3.2) | Sends `B03` if an SOS is active |
| **Sheltering in place** | `all_home` | `MECP/2/P03` *Sheltering in place* | security Yellow | — |
| **Evacuating** | `evacuated` | `MECP/2/P02` *Evacuating toward* (destination text optional) | security Yellow | — |
| **Someone missing** | `one_missing` | `MECP/1/P07` *Group separated* | security Red | — |
| **Lost** | `needs_help` | `MECP/1/P06` *Lost* | security Yellow | — |
| **Stranded** | `needs_help` | `MECP/1/P01` *Stranded / stuck* | mobility Red | — |
| **Threat nearby** | `needs_help` | `MECP/1/X01` *Dangerous person / threat nearby* | security Red | — |
| **Need supplies** | `needs_help` | `MECP/1/S0x` chosen in one follow-up tap (water, food, medication, power, fuel, tools) | preparedness Red | — |
| **Medical help needed** | `needs_help` | `MECP/0/M01` *Injury* (or another `M` code in one follow-up tap) | medical Red | — |
| **SOS** | `needs_help` | `MECP/0/B01` (already sent by the SOS runtime) | medical Red, security Red | Starts the existing SOS fan-out (countdown, location, PIN to cancel) |

Notes:

- Severity follows MECP: `0` Mayday, `1` Urgent, `2` Safety.
- Every derived MECP event carries the household tag `#HH_<household-id>` when a
  household is set, so a peer can link it to the household card. SOS keeps its
  `#SOS_<incident>` tag.
- B04 v1 has only four status values, so several situations share `needs_help`, and
  *Sheltering* uses `all_home`. The MECP event tells the team the exact situation. A
  later B04 v2 could carry the situation itself; this design does not need it.
- The follow-up tap for *Need supplies* and *Medical* is optional. Without it, the
  runtime sends the generic code (`S01`, `M01`).

### 3.1 One runtime command

```
set_situation(situation, detail: Option<String>) -> SituationRecord
```

In Rust (`node/household_situation.rs`), like `publish_community_status` today:

1. Load the stored situation, the household mode (3.5) and the device's current EAM.
2. Build the MECP event and the EAM update from the table above, and the B04 record
   only if this phone reports for the household.
3. Store them locally in one app-state transaction, so the device's own screens agree
   immediately.
4. Queue replication for each through its existing path and traffic class (B04 →
   `CommunityStatus`, event → `Event`, EAM → `Eam`, SOS → `Sos`). Queue, do not wait
   for delivery: the same lesson as the marker and SOS fixes.
5. Return the stored situation with a per-message outcome (`queued`, `held by power
   saver`, `skipped: unchanged`, `skipped: household off`).

The table lives in one Rust module, so the mapping has exactly one owner.
`packages/node-client` gains the typed call; the UI only sends the choice.

### 3.2 Not trampling the EAM

EAM factors also get set by hand on the EAM screen. The situation must not erase
those.

- The runtime remembers **which factors the current situation set** and what they
  were before.
- When the situation changes, it restores only those factors, then applies the new
  situation's factors.
- If the operator edited a factor by hand after the situation set it, the hand value
  wins and is not restored.
- A situation never lowers a factor the operator set Red by hand. For example,
  *Sheltering* does not turn a hand-set Red security into Yellow.

### 3.3 Power saver

Saver admits B04 (one transition) and SOS, but denies EAM and ordinary events.

- B04 always goes out, so Inner and Outer Circle peers learn the situation.
- The MECP event and EAM are **stored and held**. They are sent when saver ends.
  Before then, a newer situation replaces any held messages that were not yet sent,
  so the team never receives an outdated sequence.
- *SOS* behaves exactly as today: SOS traffic is always admitted.

### 3.4 Repeated taps and spam

- Choosing the current situation again sends nothing new, except *SOS*, which
  re-broadcasts as the SOS runtime already does.
- B04 keeps its existing replay and freshness rules. MECP events use a stable UID per
  device and situation change (`rem-sit-v1:<identity>:<n>`), so peers update one
  record instead of collecting duplicates.

### 3.5 Household mode: off, this phone reports, or another phone reports

The household is optional. Settings › Identity gets one setting with three values:

| Household mode | Household card | B04 sent | Situations |
|---|---|---|---|
| **Off** | Hidden. Household fields (name, ID, counts, roles) hidden in Settings. | Never | Still available as **My situation**: sends the MECP event and EAM only |
| **This phone reports for the household** (default when a household profile is set) | Shown, editable | Yes | Full fan-out, including B04 |
| **Another phone reports for the household** | Shown read-only with the status received from the reporting phone (matched by household ID) | Never | Available as **My situation**: MECP event and EAM only, tagged with the household ID |

Rules:

- Only one phone per household sends B04. The runtime refuses to publish B04 unless
  the mode is *This phone reports*.
- If B04 arrives from another identity with the **same household ID** while this
  phone is set to report, the card shows a one-line warning ("Another phone is also
  reporting for Harbour House") with a button to switch this phone to *Another phone
  reports*. Nothing is resolved silently.
- Turning the household **off** stops future B04. Peers keep the last one until it
  expires under the existing 7-day rule. Turning it back on sends nothing until the
  next situation is chosen.
- Signed Block Code import keeps setting the household profile; it does not change
  the mode on a phone where the operator turned the household off.

## 4. What changes in the app

| Place | Today | Proposed |
|---|---|---|
| Dashboard household card | Badge with four B04 choices | Badge shows the current **situation**; *Change* lists the ten situations; one tap sends everything. A short line says what went out ("Shared with community and team"). Read-only on a phone where another phone reports; hidden when the household is off, replaced by a smaller **My situation** card. |
| Settings › Identity | Household profile always shown | Household mode (Off / This phone reports / Another phone reports) at the top of the household section; profile fields hidden when Off |
| EAM screen | Six factors set by hand | Unchanged; factors set by the situation show a small "from situation" tag |
| Events timeline | B04 listed as an event | B04 hidden from the timeline. The derived MECP event (P07, P02, …) is what the team sees there. |
| Peers roster and other households | B04 status | B04 status plus, for team members, the matching MECP event's label |

## 5. Phasing

1. **Runtime command, table and household mode.** `set_situation`, the situation
   table, EAM factor bookkeeping, saver hold queue, household mode with the
   single-reporter rule and duplicate-reporter warning. Rust tests for every table row,
   transitions, hand-edit protection, saver hold/replace, and each household mode.
   UniFFI, JNI and node-client contract; regenerated bindings and native rebuild.
2. **Dashboard, Settings and Events.** The single-choice card with ten situations, the
   follow-up tap for supplies and medical, the household mode setting, the read-only
   and *My situation* variants, B04 hidden from Events, the "from situation" tag on
   EAM. Playwright coverage for each situation and each household mode.
3. **SOS unification.** *SOS* on the card triggers the existing SOS flow; ending the
   SOS returns to the previous situation or *All OK*.

## 6. Decisions taken (2026-10-19)

- Situations: the seven originally proposed plus *Lost* (`P06`), *Stranded* (`P01`)
  and *Threat nearby* (`X01`).
- *All OK* always sends `R07`.
- Mobility means the ability to move; *Evacuating* and *Sheltering* set security
  Yellow and leave mobility alone. *Someone missing* sets security Red for the same
  reason.
- One phone per household sends B04; other phones in the household send only their
  own MECP event and EAM.
- The household is optional and can be turned off.
- *Lost* and *Threat nearby* show Outer Circle peers a plain `needs_help` until a B04
  v2 can carry the exact situation.
- With the household off, *My situation* events reach only the team.

No open questions remain for phase 1.
