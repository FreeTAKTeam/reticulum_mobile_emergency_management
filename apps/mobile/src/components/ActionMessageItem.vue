<script setup lang="ts">
import { computed, nextTick, ref, shallowRef, watch } from "vue";

import StatusPill from "./StatusPill.vue";

import { useMessagesStore } from "../stores/messagesStore";
import type { ActionMessage } from "../types/domain";
import {
  ACTION_MESSAGE_STATUS_CONFIG,
  type ActionMessageStatusField,
} from "../utils/actionMessageStatus";
import { formatR3aktTeamColor } from "../utils/r3akt";

const props = defineProps<{
  message: ActionMessage;
  editable: boolean;
  selected?: boolean;
}>();

const emit = defineEmits<{
  edit: [callsign: string];
  delete: [callsign: string];
  cycle: [callsign: string, field: keyof ActionMessage];
}>();

const messagesStore = useMessagesStore();
const isExpanded = shallowRef(false);
const lastDeleteReleaseAt = shallowRef(0);
const itemElement = ref<HTMLElement | null>(null);

const formattedTeam = computed(() => formatR3aktTeamColor(props.message.groupName));
const readiness = computed(() => messagesStore.eamReadinessForCallsign(props.message.callsign));
const overallScore = computed(() => readiness.value?.overallScore ?? 0);
const overallColor = computed(() => readiness.value?.overallRingColor ?? "#ff3648");
const overallBand = computed(() => readiness.value?.overallBand ?? "Unknown");
const ringOffset = computed(() => 276.46 - ((276.46 * overallScore.value) / 100));
const toggleLabel = computed(() => (isExpanded.value ? "Hide statuses" : "Show statuses"));
const overallTitle = computed(() => `Overall readiness ${overallScore.value}% (${overallBand.value})`);
const reporterLabel = computed(() => {
  const value = props.message.reportedBy?.trim() || props.message.source?.display_name?.trim();
  return value ? `Reported by ${value}` : "";
});
const syncedLabel = computed(() => {
  const timestamp = props.message.lastSyncedAt ?? props.message.updatedAt;
  if (!timestamp) {
    return "";
  }
  return new Intl.DateTimeFormat(undefined, {
    hour: "numeric",
    minute: "2-digit",
    year: "numeric",
    month: "short",
    day: "numeric",
  }).format(timestamp);
});
const syncLabel = computed(() => {
  if (!props.message.syncState || props.message.syncState === "synced") {
    return "";
  }
  return props.message.syncState === "draft"
    ? "Draft"
    : props.message.syncState === "syncing"
      ? "Syncing"
      : "Sync error";
});

function toggleStatuses(): void {
  isExpanded.value = !isExpanded.value;
}

function cycleStatus(field: ActionMessageStatusField): void {
  if (!props.editable) {
    return;
  }
  emit("cycle", props.message.callsign, field);
}

function requestDelete(event?: Event): void {
  event?.preventDefault();
  event?.stopPropagation();
  emit("delete", props.message.callsign);
}

function requestDeleteFromRelease(event: PointerEvent | TouchEvent): void {
  if (event instanceof PointerEvent && event.pointerType === "mouse" && event.button !== 0) {
    return;
  }
  lastDeleteReleaseAt.value = Date.now();
  requestDelete(event);
}

function requestDeleteFromClick(event: MouseEvent): void {
  if (Date.now() - lastDeleteReleaseAt.value < 350) {
    event.preventDefault();
    event.stopPropagation();
    return;
  }
  requestDelete(event);
}

watch(
  () => props.selected,
  async (selected) => {
    if (!selected) {
      return;
    }
    isExpanded.value = true;
    await nextTick();
    itemElement.value?.scrollIntoView({ block: "center" });
  },
  { immediate: true },
);
</script>

<template>
  <article ref="itemElement" class="item" :class="{ selected: props.selected }">
    <header class="item-header">
      <div class="identity">
        <div class="identity-copy">
          <p class="eyebrow">Call Sign</p>
          <div class="callsign-row">
            <h3 class="callsign">{{ props.message.callsign }}</h3>
            <div class="item-actions" role="group" aria-label="Message actions">
              <button
                v-if="props.editable"
                class="action edit"
                type="button"
                :aria-label="`Edit ${props.message.callsign}`"
                title="Edit"
                @click="emit('edit', props.message.callsign)"
              >
                <svg class="action-icon" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                  <path d="M12 20h9" />
                  <path d="m16.5 3.5 4 4L8 20l-4 1 1-4z" />
                </svg>
              </button>
              <button
                class="action delete"
                type="button"
                :aria-label="`Delete ${props.message.callsign}`"
                title="Delete"
                @click="requestDeleteFromClick"
                @pointerup="requestDeleteFromRelease"
                @touchend="requestDeleteFromRelease"
              >
                <svg class="action-icon" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                  <path d="M3 6h18" />
                  <path d="M8 6V4h8v2" />
                  <path d="M19 6l-1 14H6L5 6" />
                  <path d="M10 11v5" />
                  <path d="M14 11v5" />
                </svg>
              </button>
            </div>
          </div>
          <p class="group">
            Team: {{ formattedTeam }}
            <span v-if="syncLabel" class="sync-chip">{{ syncLabel }}</span>
            <span v-else-if="props.message.lastSyncedAt" class="sync-chip sync-chip-success">Synced</span>
            <span v-if="!props.editable" class="sync-chip sync-chip-muted">Read only</span>
          </p>
          <p v-if="reporterLabel || syncedLabel" class="meta">
            <span v-if="reporterLabel">{{ reporterLabel }}</span>
            <span v-if="reporterLabel && syncedLabel" aria-hidden="true"> | </span>
            <span v-if="syncedLabel">Updated {{ syncedLabel }}</span>
          </p>
        </div>

        <div class="overall" :style="{ '--overall-color': overallColor }" :title="overallTitle">
          <svg class="overall-chart" viewBox="0 0 120 120" aria-hidden="true">
            <circle class="overall-ring-bg" cx="60" cy="60" r="44" />
            <circle
              class="overall-ring-fg"
              cx="60"
              cy="60"
              r="44"
              :style="{ '--ring-offset': ringOffset }"
            />
          </svg>
          <div class="overall-copy">
            <p class="overall-label">Overall</p>
            <p class="overall-value">{{ overallScore }}%</p>
            <p class="overall-band">{{ overallBand }}</p>
          </div>
        </div>
      </div>

      <div class="controls">
        <button
          class="status-toggle"
          type="button"
          :aria-expanded="isExpanded"
          @click="toggleStatuses"
        >
          <span>{{ toggleLabel }}</span>
          <svg class="toggle-icon" :class="{ open: isExpanded }" viewBox="0 0 24 24" fill="none">
            <path d="M7 10.5 12 15.5 17 10.5" />
          </svg>
        </button>
      </div>
    </header>

    <section v-show="isExpanded" class="status-grid">
      <button
        v-for="status in ACTION_MESSAGE_STATUS_CONFIG"
        :key="status.field"
        type="button"
        class="pill-button"
        :disabled="!props.editable"
        :title="props.editable ? undefined : 'Only your own action message can be edited.'"
        @click="cycleStatus(status.field)"
      >
        <StatusPill :label="status.label" :value="props.message[status.field]" />
      </button>
    </section>
  </article>
</template>

<style scoped src="./ActionMessageItem.css"></style>
