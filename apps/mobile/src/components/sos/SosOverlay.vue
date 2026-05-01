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
    aria-label="Trigger SOS emergency"
    @pointerdown="startPress($event, 'button')"
    @pointermove="movePress"
    @pointerup="endPress"
    @pointercancel="cancelPress"
  >
    <span class="sos-fab-shell" aria-hidden="true">
      <span class="sos-fab-screw sos-fab-screw-left"></span>
      <span class="sos-fab-screw sos-fab-screw-right"></span>
      <span class="sos-fab-face">
        <svg class="sos-fab-icon" viewBox="0 0 24 24" fill="none">
          <path d="M12 4 21 20H3L12 4Z" />
          <path d="M12 9v4" />
          <path d="M12 16.5h.01" />
        </svg>
        <span class="sos-fab-label">SOS</span>
        <span class="sos-fab-vent">
          <span></span>
          <span></span>
          <span></span>
        </span>
      </span>
    </span>
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
  background: transparent;
  border: 0;
  box-shadow: none;
  color: #fff;
  display: inline-flex;
  filter: drop-shadow(0 0 16px rgb(255 53 40 / 44%));
  height: 6.25rem;
  justify-content: center;
  letter-spacing: 0;
  padding: 0;
  width: 4.5rem;
}

.sos-fab-shell {
  align-items: center;
  background:
    linear-gradient(135deg, rgb(255 87 70 / 95%), rgb(104 12 12 / 92%) 28%, rgb(31 34 43 / 96%) 29%, rgb(13 16 24 / 98%) 42%, rgb(31 34 43 / 96%) 58%, rgb(103 12 12 / 92%) 72%, rgb(255 77 55 / 95%)),
    linear-gradient(180deg, #242833, #090d15 64%, #171b24);
  border: 1px solid rgb(255 85 62 / 88%);
  border-radius: 1.3rem;
  box-shadow:
    0 0 0 1px rgb(255 56 42 / 54%),
    0 0 18px rgb(255 50 37 / 76%),
    0 0 38px rgb(255 50 37 / 42%),
    0 14px 26px rgb(0 0 0 / 48%),
    inset 0 1px 0 rgb(255 255 255 / 18%),
    inset 0 -10px 18px rgb(0 0 0 / 50%);
  clip-path: polygon(50% 0, 82% 0, 100% 20%, 100% 72%, 74% 100%, 26% 100%, 0 72%, 0 20%, 18% 0);
  display: grid;
  height: 100%;
  justify-items: center;
  place-content: center;
  position: relative;
  width: 100%;
}

.sos-fab-shell::before {
  background:
    linear-gradient(180deg, rgb(255 255 255 / 18%), transparent 20%),
    linear-gradient(145deg, #2f3440, #111722 62%, #05070b);
  border: 1px solid rgb(255 255 255 / 10%);
  border-radius: 1rem;
  clip-path: polygon(50% 0, 78% 0, 96% 21%, 96% 72%, 72% 97%, 28% 97%, 4% 72%, 4% 21%, 22% 0);
  content: "";
  inset: 0.34rem;
  position: absolute;
}

.sos-fab-face {
  align-items: center;
  background:
    radial-gradient(circle at 46% 24%, rgb(255 151 128 / 40%), transparent 34%),
    linear-gradient(160deg, #ff3e31 0%, #cc1d1d 48%, #6f0a0a 100%);
  border: 2px solid rgb(255 93 70 / 88%);
  border-radius: 0.9rem;
  box-shadow:
    0 0 0 2px rgb(19 6 8 / 78%),
    0 0 14px rgb(255 45 35 / 50%),
    inset 0 1px 0 rgb(255 218 190 / 24%),
    inset 0 -9px 16px rgb(75 3 3 / 38%);
  clip-path: polygon(50% 0, 76% 0, 93% 21%, 93% 72%, 68% 95%, 32% 95%, 7% 72%, 7% 21%, 24% 0);
  display: grid;
  grid-template-rows: 1.42rem 2.3rem 1.2rem;
  height: 5.04rem;
  justify-items: center;
  padding-top: 0.42rem;
  position: relative;
  width: 3.46rem;
  z-index: 1;
}

.sos-fab-screw {
  background:
    radial-gradient(circle at 42% 38%, rgb(255 255 255 / 26%), transparent 18%),
    radial-gradient(circle, #070a10 0 42%, #39404a 45% 62%, #090c12 68%);
  border-radius: 999px;
  box-shadow: 0 0 0 1px rgb(0 0 0 / 50%);
  height: 0.28rem;
  position: absolute;
  top: 3.08rem;
  width: 0.28rem;
  z-index: 2;
}

.sos-fab-screw-left {
  left: 0.22rem;
}

.sos-fab-screw-right {
  right: 0.22rem;
}

.sos-fab-icon {
  height: 1.28rem;
  stroke: #fff7ed;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.9;
  width: 1.55rem;
}

.sos-fab-label {
  color: #fff7ed;
  font-family: var(--font-ui);
  font-size: 1.32rem;
  font-weight: 900;
  line-height: 1;
  max-width: 2.45rem;
  text-align: center;
  text-shadow:
    0 1px 0 rgb(255 255 255 / 16%),
    0 2px 8px rgb(0 0 0 / 52%);
  white-space: nowrap;
}

.sos-fab-vent {
  align-items: center;
  background:
    linear-gradient(180deg, rgb(255 75 55 / 34%), rgb(88 7 7 / 62%));
  border: 1px solid rgb(255 87 65 / 46%);
  border-radius: 0.35rem 0.35rem 0.5rem 0.5rem;
  box-shadow: inset 0 1px 0 rgb(255 255 255 / 14%);
  display: grid;
  gap: 0.12rem;
  height: 1.02rem;
  justify-items: center;
  margin-top: 0.06rem;
  padding: 0.22rem 0.28rem;
  width: 1.62rem;
}

.sos-fab-vent span {
  background: rgb(45 6 6 / 78%);
  border-radius: 999px;
  box-shadow: 0 0 5px rgb(255 63 46 / 26%);
  display: block;
  height: 0.11rem;
  width: 100%;
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
