import { onMounted } from "vue";

import { useNodeStore } from "../../stores/nodeStore";
import { useSettingsRestartNotice } from "../../stores/settingsRestartNotice";
import type { NodeUiSettings } from "../../types/domain";
import type { NodeSettingsSlice } from "../../utils/settingsFormSlices";
import { changedRestartFields } from "../../utils/settingsRestartReasons";
import { useSettingsDraft } from "./useSettingsDraft";

export interface NodeSettingsFormOptions {
  /** Runs after a successful save; a returned string replaces the default feedback. */
  afterSave?: (before: NodeUiSettings) => Promise<string | undefined>;
}

/**
 * Form page backed by `nodeStore.updateSettings`: saves only this page's slice merged
 * over the current settings, and records restart reasons for the Settings banner.
 */
export function useNodeSettingsForm<D extends object>(
  slice: NodeSettingsSlice<D>,
  options: NodeSettingsFormOptions = {},
) {
  const nodeStore = useNodeStore();
  const restartNotice = useSettingsRestartNotice();

  const form = useSettingsDraft<D>({
    read: () => slice.read(nodeStore.settings, nodeStore.status.appDestinationHex ?? ""),
    fingerprint: (draft) => JSON.stringify(slice.patch(draft, nodeStore.settings)),
    validate: slice.validate,
    commit: async (draft) => {
      const before = JSON.parse(JSON.stringify(nodeStore.settings)) as NodeUiSettings;
      const restartWasPending = nodeStore.nodeConfigRestartRequired;
      await nodeStore.updateSettings(slice.patch(draft, nodeStore.settings));
      const custom = await options.afterSave?.(before);
      if (nodeStore.nodeConfigRestartRequired) {
        restartNotice.record(changedRestartFields(before, nodeStore.settings), !restartWasPending);
        return custom ?? "Saved. Restart the node to apply these changes.";
      }
      return custom ?? "Saved.";
    },
  });

  onMounted(() => {
    void nodeStore.init().catch((error: unknown) => {
      const message = `Settings initialization failed: ${error instanceof Error ? error.message : String(error)}`;
      nodeStore.setLastError(message);
      nodeStore.logUi("Warn", message);
    });
  });

  return form;
}
