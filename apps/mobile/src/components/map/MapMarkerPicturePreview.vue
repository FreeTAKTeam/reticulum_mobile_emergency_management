<script setup lang="ts">
import type { MapMarkerRecord } from "@reticulum/node-client";
import { computed, ref, watch } from "vue";

import { useMapMarkersStore } from "../../stores/mapMarkersStore";
import { mdiPathFor } from "../../utils/mapMarkerIcons";

const props = defineProps<{ marker: MapMarkerRecord }>();

const store = useMapMarkersStore();
const fullImage = ref<string | undefined>();

const picture = computed(() => props.marker.picture);
const status = computed(() => picture.value?.imageStatus);
const preview = computed(() => fullImage.value ?? picture.value?.thumbnailDataUrl);

// RCH answers are relayed natively (r3akt-uas #19); this only words them.
const STATUS_TEXT: Record<string, string> = {
  requested: "Asking RCH for the full image…",
  pending: "RCH has not received the full image from the drone yet.",
  unavailable: "The full image is not available on RCH.",
  unauthorized: "RCH did not allow this device to load the full image.",
  invalid: "The received image failed its integrity check.",
};
const statusText = computed(() => (status.value ? STATUS_TEXT[status.value] ?? "" : ""));
const canRequest = computed(() =>
  Boolean(picture.value?.messageId) && status.value !== "requested" && !fullImage.value,
);
const capturedAt = computed(() => {
  const value = picture.value?.capturedAt;
  const date = value ? new Date(value) : undefined;
  return date && !Number.isNaN(date.getTime()) ? date.toLocaleString() : undefined;
});

watch(
  [() => props.marker.objectDestinationHash, status],
  async ([key, next], previous) => {
    if (key !== previous?.[0]) {
      fullImage.value = undefined;
    }
    if (next === "ok" && !fullImage.value) {
      fullImage.value = await store.pictureImage(key).catch(() => undefined);
    }
  },
  { immediate: true },
);

function load(): void {
  void store.requestPicture(props.marker.objectDestinationHash).catch(() => undefined);
}
</script>

<template>
  <figure class="marker-picture" data-testid="marker-picture-preview">
    <img v-if="preview" :src="preview" :alt="fullImage ? 'Full picture' : 'Picture thumbnail'" />
    <div v-else class="marker-picture-fallback" role="img" aria-label="No picture preview received">
      <svg viewBox="0 0 24 24" aria-hidden="true"><path :d="mdiPathFor('image')" fill="currentColor" /></svg>
      <span>No preview received</span>
    </div>
    <figcaption>
      The marker shows where the camera was aimed, not a confirmed position of what the picture shows.
      <template v-if="capturedAt"> Captured {{ capturedAt }}.</template>
    </figcaption>
    <p v-if="statusText" class="marker-picture-status" role="status">{{ statusText }}</p>
    <button v-if="canRequest" type="button" :disabled="store.busy" @click="load">
      {{ status && status !== "ok" ? "Try loading full image again" : "Load full image" }}
    </button>
  </figure>
</template>

<style scoped>
.marker-picture {
  margin: 0.7rem 0 0;
}

.marker-picture img,
.marker-picture-fallback {
  aspect-ratio: 4 / 3;
  background: #020710;
  border-radius: 6px;
  display: block;
  max-height: 40vh;
  object-fit: contain;
  width: 100%;
}

.marker-picture-fallback {
  align-items: center;
  border: 2px dashed #9cb3d6;
  color: #9cb3d6;
  display: flex;
  flex-direction: column;
  justify-content: center;
}

.marker-picture-fallback svg {
  height: 2.5rem;
  width: 2.5rem;
}

.marker-picture figcaption,
.marker-picture-status {
  color: #9cb3d6;
  font-size: 0.86rem;
  margin: 0.4rem 0 0;
}

.marker-picture button {
  margin-top: 0.5rem;
  width: 100%;
}
</style>
