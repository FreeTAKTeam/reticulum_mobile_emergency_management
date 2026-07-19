<script setup lang="ts">
import { nextTick, reactive, ref, watch } from "vue";

import { MECP_CATEGORIES, type MecpCategoryCode } from "../../utils/mecp";

const props = defineProps<{
  modelValue: MecpCategoryCode;
  active: boolean;
}>();

const emit = defineEmits<{
  "update:modelValue": [category: MecpCategoryCode];
}>();

const categoryScroller = ref<HTMLElement | null>(null);
const categoryDrag = reactive({
  active: false,
  moved: false,
  pointerId: -1,
  startY: 0,
  startScrollTop: 0,
  suppressClickUntil: 0,
});

function scrollSelectedCategoryIntoView(): void {
  if (!props.active) {
    return;
  }
  const selected = categoryScroller.value?.querySelector<HTMLElement>("[data-selected='true']");
  selected?.scrollIntoView({ block: "center", behavior: "smooth" });
}

function selectNearestVisibleCategory(): void {
  const scroller = categoryScroller.value;
  if (!scroller) {
    return;
  }
  const scrollerRect = scroller.getBoundingClientRect();
  const scrollerCenter = scrollerRect.top + scrollerRect.height / 2;
  const cards = Array.from(scroller.querySelectorAll<HTMLElement>("[data-category]"));
  let nearest: HTMLElement | null = null;
  let nearestDistance = Number.POSITIVE_INFINITY;
  for (const card of cards) {
    const rect = card.getBoundingClientRect();
    const distance = Math.abs(rect.top + rect.height / 2 - scrollerCenter);
    if (distance < nearestDistance) {
      nearest = card;
      nearestDistance = distance;
    }
  }
  const category = nearest?.dataset.category as MecpCategoryCode | undefined;
  if (category && category !== props.modelValue) {
    emit("update:modelValue", category);
  }
}

function selectCategory(category: MecpCategoryCode): void {
  if (Date.now() < categoryDrag.suppressClickUntil) {
    return;
  }
  emit("update:modelValue", category);
}

function startCategoryDrag(event: PointerEvent): void {
  if (event.pointerType === "mouse" && event.button !== 0) {
    return;
  }
  const scroller = categoryScroller.value;
  if (!scroller) {
    return;
  }
  categoryDrag.active = true;
  categoryDrag.moved = false;
  categoryDrag.pointerId = event.pointerId;
  categoryDrag.startY = event.clientY;
  categoryDrag.startScrollTop = scroller.scrollTop;
  scroller.setPointerCapture(event.pointerId);
}

function moveCategoryDrag(event: PointerEvent): void {
  if (!categoryDrag.active || event.pointerId !== categoryDrag.pointerId) {
    return;
  }
  const scroller = categoryScroller.value;
  if (!scroller) {
    return;
  }
  const deltaY = event.clientY - categoryDrag.startY;
  if (Math.abs(deltaY) > 4) {
    categoryDrag.moved = true;
    categoryDrag.suppressClickUntil = Date.now() + 250;
  }
  scroller.scrollTop = categoryDrag.startScrollTop - deltaY;
  event.preventDefault();
}

function stopCategoryDrag(event: PointerEvent): void {
  if (!categoryDrag.active || event.pointerId !== categoryDrag.pointerId) {
    return;
  }
  const scroller = categoryScroller.value;
  if (scroller?.hasPointerCapture(event.pointerId)) {
    scroller.releasePointerCapture(event.pointerId);
  }
  if (categoryDrag.moved) {
    selectNearestVisibleCategory();
  }
  categoryDrag.active = false;
  categoryDrag.moved = false;
  categoryDrag.pointerId = -1;
}

watch(
  () => [props.modelValue, props.active],
  () => {
    void nextTick(scrollSelectedCategoryIntoView).catch((error: unknown) => {
      console.warn("MECP category positioning failed.", error);
    });
  },
  { immediate: true },
);
</script>

<template>
  <div class="field-block">
    <span class="field-label">Category</span>
    <div
      ref="categoryScroller"
      class="category-scroll"
      aria-label="MECP category selector"
      @pointerdown="startCategoryDrag"
      @pointermove="moveCategoryDrag"
      @pointerup="stopCategoryDrag"
      @pointercancel="stopCategoryDrag"
    >
      <button
        v-for="category in MECP_CATEGORIES"
        :key="category.code"
        :class="['category-card', { selected: category.code === props.modelValue }]"
        :data-category="category.code"
        :aria-label="`${category.label} category`"
        :data-selected="category.code === props.modelValue"
        type="button"
        @click="selectCategory(category.code)"
      >
        <span class="category-icon">
          <svg v-if="category.icon === 'medical'" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M12 5v14" /><path d="M5 12h14" /><path d="M6 6h12v12H6z" />
          </svg>
          <svg v-else-if="category.icon === 'terrain'" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M4 18 9 7l4 7 2-4 5 8" /><path d="M8 18h8" /><path d="M11 12h2" />
          </svg>
          <svg v-else-if="category.icon === 'weather'" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M7 17h10a4 4 0 0 0 0-8 6 6 0 0 0-11.6 2" /><path d="M8 20l2-3" /><path d="M14 20l2-3" />
          </svg>
          <svg v-else-if="category.icon === 'supplies'" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M5 9h14v10H5z" /><path d="M8 9V6h8v3" /><path d="M12 12v4" /><path d="M10 14h4" />
          </svg>
          <svg v-else-if="category.icon === 'position'" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M12 21s6-5.2 6-11a6 6 0 0 0-12 0c0 5.8 6 11 6 11Z" /><path d="M12 10h.01" />
          </svg>
          <svg v-else-if="category.icon === 'coordination'" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M6 8h6" /><path d="M12 8l5 5" /><path d="M7 17h10" /><circle cx="6" cy="8" r="2" /><circle cx="18" cy="14" r="2" />
          </svg>
          <svg v-else-if="category.icon === 'response'" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="m5 13 4 4L19 7" /><path d="M4 20h16" />
          </svg>
          <svg v-else-if="category.icon === 'drill'" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M5 8h14" /><path d="M7 8v10" /><path d="M17 8v10" /><path d="m8 6 2-2" /><path d="m14 6 2-2" />
          </svg>
          <svg v-else-if="category.icon === 'leisure'" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M5 15h14" /><path d="M8 15v4" /><path d="M16 15v4" /><path d="M7 12a5 5 0 0 1 10 0" />
          </svg>
          <svg v-else-if="category.icon === 'threat'" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M12 4 3 20h18L12 4Z" /><path d="M12 9v4" /><path d="M12 17h.01" />
          </svg>
          <svg v-else-if="category.icon === 'resources'" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M7 12 12 7l5 5" /><path d="M8 12v7h8v-7" /><path d="M10 19v-4h4v4" />
          </svg>
          <svg v-else viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M12 3v4" /><path d="M12 17v4" /><path d="M4 12h4" /><path d="M16 12h4" /><circle cx="12" cy="12" r="4" />
          </svg>
        </span>
        <span class="category-copy">
          <strong>{{ category.label }}</strong>
          <small>{{ category.code }} category</small>
        </span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.field-block {
  display: grid;
  gap: 0.36rem;
  position: relative;
}

.field-label {
  color: #8da7cd;
  font-family: var(--font-ui);
  font-size: 0.72rem;
  font-weight: 700;
  letter-spacing: 0.11em;
  text-transform: uppercase;
}

.category-scroll {
  -webkit-overflow-scrolling: touch;
  cursor: grab;
  display: grid;
  gap: 0.48rem;
  max-height: 5.9rem;
  overscroll-behavior-y: contain;
  overflow-y: auto;
  padding: 0.12rem 0.28rem 0.12rem 0;
  scrollbar-color: #37c9ff rgb(7 25 54 / 84%);
  touch-action: pan-y;
  user-select: none;
}

.category-scroll:active {
  cursor: grabbing;
}

.category-card {
  align-items: center;
  background: rgb(8 22 50 / 82%);
  border: 1px solid rgb(75 118 185 / 44%);
  border-radius: 12px;
  color: #91b2df;
  cursor: pointer;
  display: grid;
  gap: 0.72rem;
  grid-template-columns: auto minmax(0, 1fr);
  min-height: 4.8rem;
  padding: 0.7rem 0.78rem;
  text-align: left;
}

.category-card.selected {
  background:
    radial-gradient(circle at 18% 22%, rgb(35 159 255 / 20%), transparent 44%),
    rgb(8 22 50 / 92%);
  border-color: rgb(102 219 255 / 78%);
  box-shadow:
    inset 0 1px 0 rgb(183 235 255 / 8%),
    0 0 20px rgb(40 178 255 / 16%);
  color: #8fe3ff;
}

.category-icon {
  align-items: center;
  background: rgb(5 18 40 / 88%);
  border: 1px solid rgb(93 171 255 / 28%);
  border-radius: 10px;
  display: inline-flex;
  height: 2.5rem;
  justify-content: center;
  width: 2.5rem;
}

.category-icon svg {
  fill: none;
  height: 1.35rem;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.75;
  width: 1.35rem;
}

.category-copy {
  display: grid;
  gap: 0.16rem;
  min-width: 0;
}

.category-copy strong {
  color: #e6f8ff;
  font-family: var(--font-body);
  font-size: 0.98rem;
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.category-copy small {
  color: #8ea8d1;
  font-family: var(--font-ui);
  font-size: 0.7rem;
  letter-spacing: 0.05em;
  overflow: hidden;
  text-overflow: ellipsis;
  text-transform: uppercase;
  white-space: nowrap;
}
</style>
