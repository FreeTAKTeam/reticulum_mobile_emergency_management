<script setup lang="ts">
import { computed, onMounted, reactive } from "vue";

import { useSosStore } from "../../stores/sosStore";

const TAP_SLOP_PX = 8;
const sosStore = useSosStore();
const drag = reactive({
  target: "" as "button" | "pill" | "",
  pointerId: -1,
  startX: 0,
  startY: 0,
  originX: 0,
  originY: 0,
  moved: false,
});

const buttonStyle = computed(() => ({
  left: `${sosStore.settings.floatingButtonX}px`,
  top: `${sosStore.settings.floatingButtonY}px`,
}));
const pillStyle = computed(() => ({
  left: `${sosStore.settings.activePillX}px`,
  top: `${sosStore.settings.activePillY}px`,
}));
const deactivationRequiresPin = computed(() => Boolean(sosStore.settings.deactivationPinHash));

function actionErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function reportActionError(error: unknown): void {
  const message = sosStore.lastError || actionErrorMessage(error);
  if (message) {
    window.alert(message);
  }
}

async function triggerFromFloatingButton(): Promise<void> {
  try {
    await sosStore.trigger("FloatingButton");
  } catch (error: unknown) {
    reportActionError(error);
  }
}

async function deactivateFromOverlay(): Promise<void> {
  const pin = deactivationRequiresPin.value
    ? window.prompt("Enter SOS PIN to deactivate")?.trim()
    : undefined;
  if (deactivationRequiresPin.value && !pin) {
    return;
  }
  try {
    await sosStore.deactivate(pin);
  } catch (error: unknown) {
    reportActionError(error);
  }
}

async function toggleFromFloatingButton(): Promise<void> {
  if (sosStore.active) {
    await deactivateFromOverlay();
    return;
  }
  await triggerFromFloatingButton();
}

async function persistPosition(target: "button" | "pill", x: number, y: number): Promise<void> {
  const next = { ...sosStore.settings };
  if (target === "button") {
    next.floatingButtonX = x;
    next.floatingButtonY = y;
  } else {
    next.activePillX = x;
    next.activePillY = y;
  }
  await sosStore.saveSettings(next);
}

function startPress(event: PointerEvent, target: "button" | "pill"): void {
  const x = target === "button" ? sosStore.settings.floatingButtonX : sosStore.settings.activePillX;
  const y = target === "button" ? sosStore.settings.floatingButtonY : sosStore.settings.activePillY;
  drag.target = target;
  drag.pointerId = event.pointerId;
  drag.startX = event.clientX;
  drag.startY = event.clientY;
  drag.originX = x;
  drag.originY = y;
  drag.moved = false;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}

function movePress(event: PointerEvent): void {
  if (!drag.target || event.pointerId !== drag.pointerId) {
    return;
  }
  const deltaX = event.clientX - drag.startX;
  const deltaY = event.clientY - drag.startY;
  if (!drag.moved && Math.hypot(deltaX, deltaY) < TAP_SLOP_PX) {
    return;
  }
  drag.moved = true;
  const maxX = Math.max(0, window.innerWidth - 96);
  const maxY = Math.max(0, window.innerHeight - 96);
  const x = Math.min(maxX, Math.max(0, drag.originX + deltaX));
  const y = Math.min(maxY, Math.max(0, drag.originY + deltaY));
  if (drag.target === "button") {
    sosStore.settings.floatingButtonX = x;
    sosStore.settings.floatingButtonY = y;
  } else {
    sosStore.settings.activePillX = x;
    sosStore.settings.activePillY = y;
  }
}

function releasePointer(event: PointerEvent): void {
  const target = event.currentTarget as HTMLElement;
  if (target.hasPointerCapture(event.pointerId)) {
    target.releasePointerCapture(event.pointerId);
  }
}

function resetPress(): void {
  drag.target = "";
  drag.pointerId = -1;
  drag.moved = false;
}

function cancelPress(event: PointerEvent): void {
  if (event.pointerId !== drag.pointerId) {
    return;
  }
  releasePointer(event);
  resetPress();
}

function endPress(event: PointerEvent): void {
  if (!drag.target || event.pointerId !== drag.pointerId) {
    return;
  }
  releasePointer(event);
  const target = drag.target;
  const wasDragged = drag.moved;
  const x = target === "button" ? sosStore.settings.floatingButtonX : sosStore.settings.activePillX;
  const y = target === "button" ? sosStore.settings.floatingButtonY : sosStore.settings.activePillY;
  resetPress();
  if (wasDragged) {
    void persistPosition(target, x, y);
    return;
  }
  void (target === "button" ? toggleFromFloatingButton() : deactivateFromOverlay());
}

onMounted(() => {
  void sosStore.init();
});
</script>

<template>
  <button
    v-if="sosStore.settings.enabled && sosStore.settings.floatingButton"
    class="sos-fab"
    :style="buttonStyle"
    type="button"
    @pointerdown="startPress($event, 'button')"
    @pointermove="movePress"
    @pointerup="endPress"
    @pointercancel="cancelPress"
  >
    SOS
  </button>

  <button
    v-if="sosStore.active"
    class="sos-pill"
    :style="pillStyle"
    type="button"
    @pointerdown="startPress($event, 'pill')"
    @pointermove="movePress"
    @pointerup="endPress"
    @pointercancel="cancelPress"
  >
    SOS ACTIVE
  </button>
</template>

<style scoped>
.sos-fab,
.sos-pill {
  border: 1px solid #ef4444;
  box-shadow: 0 12px 24px rgb(0 0 0 / 35%);
  cursor: grab;
  position: fixed;
  touch-action: none;
  z-index: 40;
}

.sos-fab {
  align-items: center;
  background: #b91c1c;
  border-radius: 8px;
  color: #fff;
  display: inline-flex;
  font-weight: 900;
  height: 4.5rem;
  justify-content: center;
  letter-spacing: 0;
  width: 4.5rem;
}

.sos-pill {
  background: #450a0a;
  border-radius: 8px;
  color: #fecaca;
  font-weight: 900;
  letter-spacing: 0;
  padding: 0.55rem 0.85rem;
}
</style>
