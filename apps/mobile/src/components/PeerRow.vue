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

const stateLabel = computed(() => {
  if (props.peer.state === "connected") {
    return "Connected";
  }
  if (props.peer.state === "connecting") {
    return "Connecting";
  }
  return "Disconnected";
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
        {{ stateLabel }} | last seen {{ new Date(props.peer.lastSeenAt).toLocaleTimeString() }}
      </p>
      <p class="details" v-if="props.peer.hops !== undefined">
        {{ props.peer.hops }} hops | {{ props.peer.verifiedCapability ? "verified" : "unverified" }}
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
        @click="emit('connectToggle', props.peer.destination, props.peer.state !== 'connected')"
      >
        {{ props.peer.state === "connected" ? "Disconnect" : "Connect" }}
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
  border-radius: 10px;
  cursor: pointer;
  font-family: var(--font-body);
  font-size: 0.9rem;
  font-weight: 700;
  min-width: 92px;
  padding: 0.42rem 0.7rem;
}

.save {
  background: rgb(17 56 98 / 72%);
  border: 1px solid rgb(78 166 255 / 72%);
  color: #74beff;
}

.connect {
  background: rgb(32 57 24 / 68%);
  border: 1px solid rgb(75 196 116 / 72%);
  color: #90dfa8;
}

@media (max-width: 860px) {
  .row {
    grid-template-columns: 1fr;
  }
}
</style>
