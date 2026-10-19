import { computed, reactive, ref, watch } from "vue";
import { onBeforeRouteLeave } from "vue-router";

import { describeSettingsSaveError, type SettingsSaveField } from "../../utils/settingsSaveErrors";

export interface SettingsDraftOptions<D extends object> {
  /** Fresh draft built from the persisted settings. */
  read: () => D;
  /** Normalized comparable form of a draft; equal fingerprints mean "nothing to save". */
  fingerprint: (draft: D) => string;
  /** Immediate input feedback; the runtime validates again on save. */
  validate?: (draft: D) => string | undefined;
  /** Persists the draft and returns the success message. */
  commit: (draft: D) => Promise<string>;
}

/**
 * Draft lifecycle for one Settings form page: dirty tracking, Save / Discard,
 * and a Discard / Keep editing prompt when the user leaves with unsaved changes.
 */
export function useSettingsDraft<D extends object>(options: SettingsDraftOptions<D>) {
  const draft = reactive(options.read()) as D;
  const saving = ref(false);
  const feedback = ref("");
  const errorField = ref<SettingsSaveField | undefined>(undefined);
  const leavePending = ref(false);
  let resolveLeave: ((leave: boolean) => void) | undefined;

  const baseline = computed(() => options.fingerprint(options.read()));
  const validationError = computed(() => options.validate?.(draft));
  const dirty = computed(
    () => options.fingerprint(draft) !== baseline.value || Boolean(validationError.value),
  );

  function discard(): void {
    Object.assign(draft, options.read());
    errorField.value = undefined;
  }

  // Persisted settings can change underneath a pristine form (store init, an import):
  // follow them, but never overwrite what the user is editing.
  watch(baseline, (_next, previous) => {
    if (options.fingerprint(draft) === previous) {
      discard();
    }
  });

  async function save(): Promise<boolean> {
    if (!dirty.value || saving.value) {
      return false;
    }
    if (validationError.value) {
      feedback.value = validationError.value;
      return false;
    }
    saving.value = true;
    feedback.value = "";
    errorField.value = undefined;
    try {
      feedback.value = await options.commit(draft);
    } catch (error: unknown) {
      const failure = describeSettingsSaveError(error);
      feedback.value = failure.message;
      errorField.value = failure.field;
      return false;
    } finally {
      saving.value = false;
    }
    discard();
    return true;
  }

  onBeforeRouteLeave(() => {
    if (!dirty.value) {
      return true;
    }
    leavePending.value = true;
    return new Promise<boolean>((resolve) => {
      resolveLeave = resolve;
    });
  });

  function answerLeave(leave: boolean): void {
    leavePending.value = false;
    if (leave) {
      discard();
    }
    resolveLeave?.(leave);
    resolveLeave = undefined;
  }

  return {
    answerLeave,
    dirty,
    discard,
    draft,
    errorField,
    feedback,
    leavePending,
    save,
    saving,
    validationError,
  };
}

export type SettingsDraft<D extends object> = ReturnType<typeof useSettingsDraft<D>>;
