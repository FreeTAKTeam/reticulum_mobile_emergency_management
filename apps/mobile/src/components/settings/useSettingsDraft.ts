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
  /** Draft fingerprint the current feedback describes; any later edit makes it stale. */
  let feedbackFingerprint = "";

  const baseline = computed(() => options.fingerprint(options.read()));
  const validationError = computed(() => options.validate?.(draft));
  const dirty = computed(
    () => options.fingerprint(draft) !== baseline.value || Boolean(validationError.value),
  );

  function resetDraft(): void {
    Object.assign(draft, options.read());
  }

  function showFeedback(message: string, field?: SettingsSaveField): void {
    feedback.value = message;
    errorField.value = field;
    feedbackFingerprint = options.fingerprint(draft);
  }

  function discard(): void {
    resetDraft();
    showFeedback("");
  }

  // Persisted settings can change underneath a pristine form (store init, an import):
  // follow them, but never overwrite what the user is editing.
  watch(baseline, (_next, previous) => {
    if (options.fingerprint(draft) === previous) {
      resetDraft();
    }
  });

  // "Saved." or an old error must not linger once the user edits the form again.
  watch(() => options.fingerprint(draft), (next) => {
    if ((feedback.value || errorField.value) && next !== feedbackFingerprint) {
      showFeedback("");
    }
  });

  async function save(): Promise<boolean> {
    if (!dirty.value || saving.value) {
      return false;
    }
    if (validationError.value) {
      showFeedback(validationError.value);
      return false;
    }
    saving.value = true;
    showFeedback("");
    let message: string;
    try {
      message = await options.commit(draft);
    } catch (error: unknown) {
      const failure = describeSettingsSaveError(error);
      showFeedback(failure.message, failure.field);
      return false;
    } finally {
      saving.value = false;
    }
    resetDraft();
    showFeedback(message);
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
