<script setup lang="ts">
import { computed, ref, watch } from "vue";

import type { DiscoveredPeer } from "../types/domain";

const props = defineProps<{
  peer: DiscoveredPeer;
  isSaved: boolean;
}>();

const emit = defineEmits<{
  saveToggle: [destination: string, next: boolean];
  connectToggle: [destination: string, next: boolean];
  labelChange: [destination: string, label: string];
}>();

const localLabel = ref(props.peer.label ?? "");
watch(
  () => props.peer.label,
  (value) => {
    localLabel.value = value ?? "";
  },
);

const saveStateLabel = computed(() => (props.isSaved || props.peer.saved ? "Saved" : "Unsaved"));
const staleLabel = computed(() => (props.peer.stale ? "Stale" : "Current"));
const linkLabel = computed(() => (props.peer.activeLink ? "Connected" : "Not connected"));
const connectButtonDisabled = computed(() => !props.isSaved && !props.peer.saved);
const connectButtonLabel = computed(() => {
  if (props.peer.activeLink) {
    return "Disconnect";
  }
  return props.isSaved ? "Connect" : "Save first";
});
const lastSeenLabel = computed(() => {
  const lastSeenAt = props.peer.lastSeenAt;
  if (!lastSeenAt) {
    return "never";
  }
  const elapsedMs = Math.max(0, Date.now() - lastSeenAt);
  const elapsedMinutes = Math.floor(elapsedMs / 60_000);
  if (elapsedMinutes < 60) {
    return `seen ${Math.max(1, elapsedMinutes)} min ago`;
  }
  const elapsedHours = Math.floor(elapsedMinutes / 60);
  if (elapsedHours < 24) {
    return `seen ${elapsedHours} hr ago`;
  }
  const elapsedDays = Math.floor(elapsedHours / 24);
  return `seen ${elapsedDays} day${elapsedDays === 1 ? "" : "s"} ago`;
});
const resolutionErrorText = computed(() =>
  typeof props.peer.lastResolutionError === "string"
    ? props.peer.lastResolutionError.trim()
    : "",
);
const resolutionLabel = computed(() => {
  if (resolutionErrorText.value) {
    return `Resolution error: ${resolutionErrorText.value}`;
  }
  if (props.peer.lastResolutionAttemptAt) {
    return "Resolution attempted";
  }
  return "No resolution attempts";
});
</script>

<template>
  <article class="row">
    <div class="meta">
      <p class="dest">{{ props.peer.destination }}</p>
      <p class="announced-name" v-if="props.peer.announcedName">
        {{ props.peer.announcedName }}
      </p>
      <p class="details">
        {{ saveStateLabel }} | {{ linkLabel }} | {{ staleLabel }}
      </p>
      <p class="details">
        {{ lastSeenLabel }}
      </p>
      <p class="details">
        {{ resolutionLabel }}
      </p>
      <p class="details" v-if="props.peer.hops !== undefined">
        {{ props.peer.hops }} hops
      </p>
    </div>
    <label class="label-input-wrap">
      Local label
      <input
        class="label-input"
        type="text"
        :value="localLabel"
        @input="localLabel = ($event.target as HTMLInputElement).value"
        @change="emit('labelChange', props.peer.destination, localLabel)"
      />
    </label>
    <div class="actions">
      <button
        class="btn save"
        type="button"
        @click="emit('saveToggle', props.peer.destination, !props.isSaved)"
      >
        {{ props.isSaved ? "Unsave" : "Save" }}
      </button>
        <button
          class="btn connect"
          type="button"
          :disabled="connectButtonDisabled"
          @click="emit('connectToggle', props.peer.destination, !props.peer.activeLink)"
        >
          {{ connectButtonLabel }}
        </button>
    </div>
  </article>
</template>

<style scoped>
.row {
  align-items: center;
  background: rgb(12 27 58 / 74%);
  border: 1px solid rgb(78 123 196 / 26%);
  border-radius: 13px;
  display: grid;
  gap: 0.7rem;
  grid-template-columns: 1.4fr 0.9fr auto;
  padding: 0.75rem 0.9rem;
}

.meta {
  min-width: 0;
}

.dest {
  color: #ddf1ff;
  font-family: var(--font-ui);
  font-size: 0.92rem;
  letter-spacing: 0.06em;
  margin: 0;
  overflow-wrap: anywhere;
}

.announced-name {
  color: #7be4ff;
  font-family: var(--font-headline);
  font-size: 1rem;
  margin: 0.22rem 0 0;
}

.details {
  color: #8ea8d1;
  font-family: var(--font-body);
  font-size: 0.9rem;
  margin: 0.15rem 0 0;
}

.label-input-wrap {
  color: #86a0cc;
  display: grid;
  font-family: var(--font-ui);
  font-size: 0.76rem;
  letter-spacing: 0.1em;
  text-transform: uppercase;
}

.label-input {
  background: rgb(6 18 43 / 70%);
  border: 1px solid rgb(72 113 178 / 40%);
  border-radius: 9px;
  color: #d3e9ff;
  font-family: var(--font-body);
  font-size: 0.95rem;
  margin-top: 0.34rem;
  padding: 0.4rem 0.52rem;
}

.actions {
  display: flex;
  gap: 0.5rem;
}

.btn {
  --btn-bg: linear-gradient(180deg, rgb(10 35 72 / 88%), rgb(6 24 54 / 92%));
  --btn-bg-pressed: linear-gradient(180deg, rgb(196 240 255 / 96%), rgb(118 212 255 / 94%));
  --btn-border: rgb(74 133 207 / 45%);
  --btn-border-pressed: rgb(224 248 255 / 86%);
  --btn-shadow: inset 0 1px 0 rgb(209 244 255 / 10%), 0 8px 18px rgb(2 14 32 / 18%);
  --btn-shadow-pressed: inset 0 1px 0 rgb(255 255 255 / 75%), 0 4px 10px rgb(3 21 47 / 24%);
  --btn-color: #8fdbff;
  --btn-color-pressed: #042541;
  background: var(--btn-bg);
  border: 1px solid var(--btn-border);
  border-radius: 999px;
  box-shadow: var(--btn-shadow);
  color: var(--btn-color);
  cursor: pointer;
  font-family: var(--font-ui);
  font-size: 0.78rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  min-height: 32px;
  min-width: 92px;
  padding: 0 0.82rem;
  text-transform: uppercase;
}

.btn:disabled {
  cursor: not-allowed;
  opacity: 0.55;
  transform: none;
}

@media (max-width: 860px) {
  .row {
    grid-template-columns: 1fr;
  }
}
</style>
