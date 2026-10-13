---
name: REM Tactical Blue
colors:
  primary: "#64beff"
  secondary: "#9cb3d6"
  surface: "#020710"
  on-surface: "#def1ff"
  error: "#ffb4ab"
typography:
  headline:
    fontFamily: Rajdhani
    fontWeight: 600-700
  body-md:
    fontFamily: Chakra Petch
    fontSize: 16px
    fontWeight: 400
rounded:
  md: 14px
  lg: 16px
---

# Design System

## Overview
REM uses a tactical, high-contrast dark interface for emergency coordination on mobile and web.
The visual language should feel field-ready rather than consumer-social: dense enough for operational context, but still fast to scan under stress.
Cold blue highlights, strong borders, and instrument-style cards communicate status without relying on decorative effects.

## Product Character
- REM is a Reticulum-based emergency operations client for messages, events, telemetry, peer discovery, and node control.
- Screens should prioritize operational clarity, routing state, and readiness over marketing polish.
- The interface should feel resilient and tool-like: a mesh operations console adapted to a mobile-first form factor.

## Colors
- **Primary** (`#64beff`): active controls, high-priority actions, badges, selected accents
- **Secondary** (`#9cb3d6`): supporting text, metadata, summaries, secondary labels
- **Surface** (`#020710`): app background and deepest canvas tone
- **Panel Surface** (`#091937` to `#071025`): cards and control panels using layered blue-black gradients
- **On-surface** (`#def1ff`): primary text on dark backgrounds
- **Border Accent** (`rgba(74, 133, 207, 0.45)`): outlines that define controls without heavy elevation
- **Error** (`#ffb4ab`): destructive states, validation failures, and dangerous actions

## Typography
- **Headlines**: `Rajdhani`, semi-bold to bold, compact and technical in tone
- **UI Labels**: `Rajdhani`, medium to bold, uppercase or high letter-spacing where a control needs a tactical/instrument feel
- **Body**: `Chakra Petch`, regular, optimized for operational copy and status text
- **Scale**: headlines use `clamp(...)` sizing for mobile/desktop continuity; body copy typically sits around 14-16px

## Components
- **Buttons**: gradient-filled, bordered, and pressable; pressed state inverts to a brighter ice-blue treatment with a slight downward transform
- **Badges/Chips**: pill-shaped, uppercase, compact, used for counts, runtime state, and quick actions like Announce and Sync
- **Panels**: rounded tactical cards with layered gradients and subtle radial highlights instead of flat fills or heavy shadows
- **Status Rings**: used for readiness summaries; color carries meaning and should be paired with text labels/bands
- **Inputs**: dark surfaces with strong legibility, straightforward borders, and minimal ornament
- **Lists/Rows**: information-forward layouts for peers, conversations, and events; metadata should remain visually subordinate to the primary identity or status line

## Interaction Principles
- Primary actions should be obvious, but the palette should stay restrained; not every control should compete for attention.
- Press feedback must remain consistent across the app through the shared global button rules.
- Saved, connected, publishing, blocked, and unknown states should always read clearly without requiring color alone.
- Dense screens are acceptable when grouping, spacing, and typography preserve scanability.
- Mobile layouts should collapse cleanly without hiding critical node, telemetry, or messaging controls.

## Do's and Don'ts
- Do keep the tactical blue palette consistent across views.
- Do use borders and contrast to define hierarchy before adding more effects.
- Do keep operational summaries short, legible, and near the controls they describe.
- Do preserve the existing global press-feedback system instead of adding one-off button behaviors.
- Don't introduce soft consumer-app styling, pastel accents, or playful motion.
- Don't flatten panels into plain gray cards; REM should retain its instrument-like depth.
- Don't mix unrelated font families or switch away from the existing Rajdhani/Chakra Petch pairing.
- Don't hide critical runtime or routing state behind decorative UI.

## Map Markers
Point markers ("PLEs") are shared map objects using the RCH symbol catalog from `r3akt-map-core`. They are a separate layer from telemetry positions and SOS, and must stay fast to use one-handed under stress.

- **Add**: a single 48px add-marker button at the bottom right of the map, inside the thumb zone. Existing indicator chips stay at the top; a "Markers" count chip joins them.
- **Pick a symbol**: a bottom sheet lists a "Recent" row first, then every catalog symbol grouped by its RCH category, each with icon and label. Search matches catalog labels and RCH aliases (for example "car", "hospital", "uav"). Icons are the catalog's MDI names, so REM and RCH show the same symbol for the same key.
- **Place**: a fixed crosshair pin sits at the map center; the user drags the map under it, sees live coordinates, and confirms with a verb button ("Place fire"). Long-press on the map is a shortcut that places the most recent symbol at the pressed point.
- **No form on create**: names default to the symbol label with a running number ("Fire 3"). Rename later from the details sheet.
- **Inspect and edit**: tapping a marker opens a details sheet (name, category, age, origin: Local or Shared) with three large actions: Rename, Move (reuses the crosshair), Delete (with confirmation).
- **Visibility**: the layer menu has a markers on/off switch. Marker pins use bordered tactical styling distinct from telemetry and SOS markers; state is never conveyed by color alone.
- **Touch**: all marker controls are at least 44px; sheets close by drag or an explicit Cancel.
- Symbols cannot change after creation (shared RCH rule): delete and re-add instead.
