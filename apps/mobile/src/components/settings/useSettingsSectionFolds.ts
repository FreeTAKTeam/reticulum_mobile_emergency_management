import { ref } from "vue";

const STORAGE_KEY = "reticulum.mobile.settingsFoldedSections.v1";

function loadFolded(): string[] {
  try {
    const parsed: unknown = JSON.parse(window.localStorage.getItem(STORAGE_KEY) ?? "[]");
    return Array.isArray(parsed) ? parsed.filter((entry): entry is string => typeof entry === "string") : [];
  } catch {
    return [];
  }
}

/** Per-device memory of which Settings index sections are folded; all start open. */
export function useSettingsSectionFolds() {
  const folded = ref(new Set(loadFolded()));

  function isFolded(sectionId: string): boolean {
    return folded.value.has(sectionId);
  }

  function toggle(sectionId: string): void {
    const next = new Set(folded.value);
    if (next.has(sectionId)) next.delete(sectionId);
    else next.add(sectionId);
    folded.value = next;
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify([...next]));
    } catch {
      // Restricted webviews can refuse storage; folding still works for this visit.
    }
  }

  return { isFolded, toggle };
}
