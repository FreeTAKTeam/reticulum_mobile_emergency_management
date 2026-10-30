---
name: REM Tactical Light
colors:
  primary: "#1d5e17"
  secondary: "#46543f"
  surface: "#e8ecdf"
  on-surface: "#0b120a"
  error: "#b5150a"
typography:
  headline:
    fontFamily: Rajdhani
    fontWeight: 700
  body-md:
    fontFamily: Chakra Petch
    fontSize: 16px
    fontWeight: 500
rounded:
  md: 14px
  lg: 16px
---

# Design System: Outdoor Light Mode

## Overview
REM Tactical Light is the high-visibility daylight counterpart of the dark interface in `design.md`.
It exists for outdoor use: in direct sun a dark screen reflects glare and its thin bright lines wash out, so the switch inverts to dark ink on a bone-coloured surface.
It matches the REM Tactical Light watch face (`rem-tactical-trex2`), so the phone and the watch read as one system in the field.
Layout, components, and behaviour are unchanged; only colour, stroke weight, and font weight differ from the dark theme.

## When To Use
- A user-facing switch (Dark / Outdoor Light) selects the theme; it is never chosen automatically from time of day, because operators may work in shade or at night with the device in sunlight.
- The choice persists per device and applies to every view at once, including the map controls and sheets.
- The dark theme remains the default. The light theme drains more battery on OLED phones and should say so beside the switch.

## Colors
Every text colour reaches at least 5:1 contrast on the panel fill (`#f7f8f1`).

- **Primary** (`#1d5e17`, 7.4:1): active controls, labels, selected accents, high-priority actions
- **Secondary** (`#46543f`, 7.6:1): supporting text, metadata, summaries, secondary labels
- **Surface** (`#e8ecdf`): app background; may use a radial gradient from `#eef1e6` at the centre to `#c9d1b8` at the edges
- **Panel Surface** (`#f7f8f1`): cards and control panels
- **Panel Strong** (`#ffffff`): the single most important panel on a screen (for example the live status or the active message)
- **On-surface** (`#0b120a`, 17.8:1): primary text and key values
- **Border Accent** (`#22401c`): panel outlines and dividers, drawn at 2px
- **Info** (`#006f7d`, 5.5:1): normal events, informational badges
- **Blue** (`#1449a3`, 7.8:1): low-priority or friendly-state indicators
- **Caution** (`#7a5c00`, 5.9:1): caution and attention states; replaces yellow, which disappears on a light surface
- **Warning** (`#b34700`, 5.1:1): stale, offline, and connecting states
- **Error** (`#b5150a`, 6.4:1): emergency, SOS, destructive states, validation failures
- **Highlight** (`#e5570c`): safety-orange accent for a single focal mark, such as the north pointer or a new-alert marker; never for text

### Mapping From The Dark Theme
| Role | Dark (`design.md`) | Outdoor Light |
|---|---|---|
| Primary | `#64beff` | `#1d5e17` |
| Secondary | `#9cb3d6` | `#46543f` |
| Surface | `#020710` | `#e8ecdf` |
| Panel Surface | `#091937` to `#071025` | `#f7f8f1` |
| On-surface | `#def1ff` | `#0b120a` |
| Border Accent | `rgba(74, 133, 207, 0.45)` | `#22401c` at 2px |
| Error | `#ffb4ab` | `#b5150a` |

## Typography
- Keep the `Rajdhani` / `Chakra Petch` pairing from the dark theme; the switch changes colour, not identity.
- Step weights up one level outdoors: headlines and UI labels at 700, body at 500. Thin strokes break up in glare.
- Do not drop body copy below 14px in this theme.
- The watch face uses Teko SemiBold for its clock and Arial Narrow Bold for its readouts; those are watch-only and are not brought into the app.

## Components
- **Buttons**: solid fills, not gradients. Primary buttons use Primary with `#f7f8f1` text; the pressed state darkens to `#0b120a` with the same slight downward transform as the dark theme.
- **Badges/Chips**: pill-shaped and uppercase as in the dark theme, with a 2px border in the badge colour on a Panel Surface fill, so they stay legible when the screen is dimmed.
- **Panels**: flat Panel Surface fill with a 2px Border Accent outline. Drop the radial highlights; on a light surface they read as smudges.
- **Status Rings**: same meaning and colours as above, always paired with text labels.
- **Inputs**: Panel Strong fill, 2px Border Accent outline, On-surface text; the focused field takes a Primary outline.
- **Map**: map controls and sheets follow this theme; marker pins keep their tactical bordered styling with a dark outline so they separate from bright base layers.

## Do's and Don'ts
- Do keep every status readable without colour alone, exactly as in the dark theme.
- Do use 2px borders and dividers; 1px lines vanish in sunlight.
- Do test on a real phone outdoors at full and half brightness before shipping changes to this theme.
- Don't use yellow, cyan, or light tints for text or thin lines.
- Don't add shadows or soft gradients to create depth; borders define hierarchy here too.
- Don't override the theme per screen; the switch applies everywhere.
