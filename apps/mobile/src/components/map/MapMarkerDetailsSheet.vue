<script setup lang="ts">
import type { MapMarkerRecord, MapMarkerSymbolRecord } from "@reticulum/node-client";
import { computed, ref, watch } from "vue";

import { mdiPathFor } from "../../utils/mapMarkerIcons";
import {
  categoryLabel,
  formatCoordinate,
  markerAgeLabel,
  markerOriginLabel,
} from "../../utils/mapMarkerPresentation";

const props = defineProps<{
  marker: MapMarkerRecord;
  symbol: MapMarkerSymbolRecord | undefined;
  busy: boolean;
  error: string;
}>();

const emit = defineEmits<{
  rename: [name: string];
  move: [];
  delete: [];
  close: [];
}>();

const mode = ref<"view" | "rename" | "confirm-delete">("view");
const draftName = ref("");
const nameError = ref("");

watch(
  () => props.marker.objectDestinationHash,
  () => {
    mode.value = "view";
    nameError.value = "";
  },
);

const subtitle = computed(() =>
  [
    categoryLabel(props.marker.category),
    markerAgeLabel(props.marker.updatedAtMs, Date.now()),
    markerOriginLabel(props.marker),
  ].join(" · "),
);

function startRename(): void {
  draftName.value = props.marker.name;
  nameError.value = "";
  mode.value = "rename";
}

function submitRename(): void {
  const name = draftName.value.trim();
  if (!name) {
    nameError.value = "Enter a name";
    return;
  }
  emit("rename", name);
  mode.value = "view";
}
</script>

<template>
  <section class="marker-sheet" role="dialog" aria-modal="false" :aria-label="`Marker ${marker.name}`">
    <div class="marker-detail-head">
      <svg class="marker-detail-icon" viewBox="0 0 24 24" aria-hidden="true">
        <path :d="mdiPathFor(symbol?.mdi)" fill="currentColor" />
      </svg>
      <div class="marker-detail-title">
        <h2 data-testid="marker-detail-name">{{ marker.name }}</h2>
        <p>{{ subtitle }}</p>
        <p class="marker-detail-coords">{{ formatCoordinate(marker.lat, marker.lon) }}</p>
      </div>
      <button type="button" class="marker-sheet-close" aria-label="Close marker details" @click="emit('close')">
        Close
      </button>
    </div>
    <p v-if="marker.notes" class="marker-detail-notes">{{ marker.notes }}</p>
    <p v-if="error" class="marker-sheet-error" role="alert">{{ error }}</p>

    <form v-if="mode === 'rename'" class="marker-rename" @submit.prevent="submitRename">
      <input
        v-model="draftName"
        class="marker-symbol-search"
        type="text"
        maxlength="96"
        aria-label="Marker name"
        @input="nameError = ''"
      />
      <p v-if="nameError" class="marker-sheet-error" role="alert">{{ nameError }}</p>
      <div class="marker-actions two">
        <button type="button" @click="mode = 'view'">Cancel</button>
        <button type="submit" class="marker-primary" :disabled="busy">Save name</button>
      </div>
    </form>

    <div v-else-if="mode === 'confirm-delete'" class="marker-confirm">
      <p>Delete "{{ marker.name }}" from the map?</p>
      <div class="marker-actions two">
        <button type="button" @click="mode = 'view'">Keep</button>
        <button type="button" class="marker-danger" :disabled="busy" @click="emit('delete')">Delete</button>
      </div>
    </div>

    <div v-else class="marker-actions three">
      <button type="button" @click="startRename">Rename</button>
      <button type="button" @click="emit('move')">Move</button>
      <button type="button" class="marker-danger" @click="mode = 'confirm-delete'">Delete</button>
    </div>
  </section>
</template>

<style scoped>
.marker-detail-head {
  align-items: flex-start;
  display: flex;
  gap: 0.7rem;
}

.marker-detail-icon {
  color: #64beff;
  flex: 0 0 auto;
  height: 2rem;
  margin-top: 0.15rem;
  width: 2rem;
}

.marker-detail-title {
  flex: 1 1 auto;
  min-width: 0;
}

.marker-detail-title h2 {
  margin: 0;
  overflow-wrap: anywhere;
}

.marker-detail-title p {
  color: #9cb3d6;
  font-size: 0.86rem;
  margin: 0.1rem 0 0;
}

.marker-detail-coords {
  font-variant-numeric: tabular-nums;
}

.marker-detail-notes {
  color: #def1ff;
  font-size: 0.92rem;
  margin: 0.6rem 0 0;
  overflow-wrap: anywhere;
}

.marker-rename,
.marker-confirm {
  margin-top: 0.7rem;
}

.marker-confirm p {
  margin: 0 0 0.6rem;
}
</style>
