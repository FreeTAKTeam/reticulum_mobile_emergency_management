import { computed, ref, toValue, watch, type MaybeRefOrGetter } from "vue";

import { LIST_WINDOW_SIZE, listWindowBounds } from "../utils/listWindow";

export { LIST_WINDOW_SIZE } from "../utils/listWindow";

type ListWindowOptions = {
  pageSize?: number;
  fromEnd?: boolean;
  resetKey?: MaybeRefOrGetter<unknown>;
};

export function useListWindow<T>(
  source: MaybeRefOrGetter<readonly T[]>,
  options: ListWindowOptions = {},
) {
  const pageSize = Math.max(1, Math.floor(options.pageSize ?? LIST_WINDOW_SIZE));
  const fromEnd = options.fromEnd === true;
  const page = ref(0);
  const followsEnd = ref(fromEnd);
  const total = computed(() => toValue(source).length);
  const bounds = computed(() => listWindowBounds(total.value, page.value, pageSize));
  const pageCount = computed(() => bounds.value.pageCount);

  function lastPage(): number {
    return pageCount.value - 1;
  }

  function clampPage(): void {
    page.value = Math.min(Math.max(0, page.value), lastPage());
  }

  function reset(): void {
    page.value = fromEnd ? lastPage() : 0;
    followsEnd.value = fromEnd;
  }

  watch(total, () => {
    if (fromEnd && followsEnd.value) {
      page.value = lastPage();
      return;
    }
    clampPage();
  }, { immediate: true });

  if (options.resetKey !== undefined) {
    watch(() => toValue(options.resetKey), reset);
  }

  const startIndex = computed(() => bounds.value.startIndex);
  const endIndex = computed(() => bounds.value.endIndex);
  const items = computed(() => toValue(source).slice(startIndex.value, endIndex.value));
  const hasPrevious = computed(() => page.value > 0);
  const hasNext = computed(() => page.value < lastPage());

  function previous(): void {
    if (hasPrevious.value) {
      page.value -= 1;
      followsEnd.value = false;
    }
  }

  function next(): void {
    if (hasNext.value) {
      page.value += 1;
    }
    followsEnd.value = fromEnd && page.value === lastPage();
  }

  function showIndex(index: number): void {
    if (index < 0 || index >= total.value) {
      return;
    }
    page.value = Math.floor(index / pageSize);
    followsEnd.value = fromEnd && page.value === lastPage();
  }

  return {
    items,
    total,
    startIndex,
    endIndex,
    hasPrevious,
    hasNext,
    previous,
    next,
    reset,
    showIndex,
  };
}
