import { defineStore } from "pinia";
import { ref } from "vue";

/**
 * UI-only record of which saved Settings fields wait for a node restart. Whether a
 * restart is pending is owned by `nodeStore.nodeConfigRestartRequired`; this store
 * only keeps the plain-language reasons for the Settings banner.
 */
export const useSettingsRestartNotice = defineStore("settingsRestartNotice", () => {
  const reasons = ref<string[]>([]);

  /** Adds reasons; `fresh` drops reasons left over from a restart that already happened. */
  function record(next: readonly string[], fresh: boolean): void {
    const base = fresh ? [] : reasons.value;
    reasons.value = [...new Set([...base, ...next])];
  }

  function clear(): void {
    reasons.value = [];
  }

  return { reasons, record, clear };
});
