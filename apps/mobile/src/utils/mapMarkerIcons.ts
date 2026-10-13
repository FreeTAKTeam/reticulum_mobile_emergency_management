import {
  mdiAccount,
  mdiAccountGroup,
  mdiAlert,
  mdiAntenna,
  mdiCamera,
  mdiCar,
  mdiClipboardCheck,
  mdiClover,
  mdiDrone,
  mdiFire,
  mdiHomeFlood,
  mdiHospital,
  mdiMapMarker,
  mdiOfficeBuilding,
  mdiPaw,
  mdiRadar,
  mdiRadio,
  mdiRectangle,
  mdiRhombus,
  mdiSquare,
} from "@mdi/js";

// Rendering only: the catalog names each symbol's MDI icon; this maps those
// names to bundled SVG paths so icons work offline. Unknown names fall back
// to the generic marker so a newer catalog never renders blank.
const MDI_PATHS: Record<string, string> = {
  "account": mdiAccount,
  "account-group": mdiAccountGroup,
  "alert": mdiAlert,
  "antenna": mdiAntenna,
  "camera": mdiCamera,
  "car": mdiCar,
  "clipboard-check": mdiClipboardCheck,
  "clover": mdiClover,
  "drone": mdiDrone,
  "fire": mdiFire,
  "home-flood": mdiHomeFlood,
  "hospital": mdiHospital,
  "map-marker": mdiMapMarker,
  "office-building": mdiOfficeBuilding,
  "paw": mdiPaw,
  "radar": mdiRadar,
  "radio": mdiRadio,
  "rectangle": mdiRectangle,
  "rhombus": mdiRhombus,
  "square": mdiSquare,
};

export function mdiPathFor(mdiName: string | undefined): string {
  return (mdiName && MDI_PATHS[mdiName]) || mdiMapMarker;
}

export function mdiSvgMarkup(mdiName: string | undefined): string {
  return `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="${mdiPathFor(mdiName)}" fill="currentColor"/></svg>`;
}
