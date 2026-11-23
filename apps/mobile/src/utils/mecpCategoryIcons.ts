import { MECP_CATEGORIES } from "./mecp";

// Rendering only: stroke-icon bodies for a 24x24 viewBox, shared by the event
// composer and the map so a category looks the same in both. Static markup,
// safe for innerHTML.
const ICONS: Record<string, string> = {
  medical: '<path d="M12 5v14" /><path d="M5 12h14" /><path d="M6 6h12v12H6z" />',
  terrain: '<path d="M4 18 9 7l4 7 2-4 5 8" /><path d="M8 18h8" /><path d="M11 12h2" />',
  weather: '<path d="M7 17h10a4 4 0 0 0 0-8 6 6 0 0 0-11.6 2" /><path d="M8 20l2-3" /><path d="M14 20l2-3" />',
  supplies: '<path d="M5 9h14v10H5z" /><path d="M8 9V6h8v3" /><path d="M12 12v4" /><path d="M10 14h4" />',
  position: '<path d="M12 21s6-5.2 6-11a6 6 0 0 0-12 0c0 5.8 6 11 6 11Z" /><path d="M12 10h.01" />',
  coordination: '<path d="M6 8h6" /><path d="M12 8l5 5" /><path d="M7 17h10" /><circle cx="6" cy="8" r="2" /><circle cx="18" cy="14" r="2" />',
  response: '<path d="m5 13 4 4L19 7" /><path d="M4 20h16" />',
  drill: '<path d="M5 8h14" /><path d="M7 8v10" /><path d="M17 8v10" /><path d="m8 6 2-2" /><path d="m14 6 2-2" />',
  leisure: '<path d="M5 15h14" /><path d="M8 15v4" /><path d="M16 15v4" /><path d="M7 12a5 5 0 0 1 10 0" />',
  threat: '<path d="M12 4 3 20h18L12 4Z" /><path d="M12 9v4" /><path d="M12 17h.01" />',
  resources: '<path d="M7 12 12 7l5 5" /><path d="M8 12v7h8v-7" /><path d="M10 19v-4h4v4" />',
  beacon: '<path d="M12 3v4" /><path d="M12 17v4" /><path d="M4 12h4" /><path d="M16 12h4" /><circle cx="12" cy="12" r="4" />',
};
// Events that are not MECP have no category: a plain note.
const NOTE = '<path d="M6 4h9l3 3v13H6z" /><path d="M9 11h6" /><path d="M9 15h6" />';

/** SVG body for a category icon name ("medical"); unknown names get the beacon. */
export function mecpIconBody(icon: string): string {
  return ICONS[icon] ?? ICONS.beacon;
}

/** SVG body for an MECP category code ("M"); anything else gets the note icon. */
export function mecpCategoryIconBody(categoryCode: string | undefined): string {
  const category = MECP_CATEGORIES.find((candidate) => candidate.code === categoryCode);
  return category ? mecpIconBody(category.icon) : NOTE;
}
