<script setup lang="ts">
import StatusPill from "./StatusPill.vue";

import type { ActionMessage } from "../types/domain";

const props = defineProps<{
  messages: ActionMessage[];
}>();

const emit = defineEmits<{
  edit: [callsign: string];
  delete: [callsign: string];
  cycle: [callsign: string, field: keyof ActionMessage];
}>();

function cycleStatus(callsign: string, field: keyof ActionMessage): void {
  emit("cycle", callsign, field);
}
</script>

<template>
  <section class="list">
    <article v-for="message in props.messages" :key="message.callsign" class="item">
      <header class="item-header">
        <h3 class="callsign">Callsign: {{ message.callsign }}</h3>
        <p class="group">Group: {{ message.groupName }}</p>
      </header>
      <div class="pills">
        <button
          type="button"
          class="pill-button"
          @click="cycleStatus(message.callsign, 'securityStatus')"
        >
          <StatusPill label="Security" :value="message.securityStatus" />
        </button>
        <button
          type="button"
          class="pill-button"
          @click="cycleStatus(message.callsign, 'capabilityStatus')"
        >
          <StatusPill label="Capability" :value="message.capabilityStatus" />
        </button>
        <button
          type="button"
          class="pill-button"
          @click="cycleStatus(message.callsign, 'preparednessStatus')"
        >
          <StatusPill label="Preparedness" :value="message.preparednessStatus" />
        </button>
        <button
          type="button"
          class="pill-button"
          @click="cycleStatus(message.callsign, 'medicalStatus')"
        >
          <StatusPill label="Medical" :value="message.medicalStatus" />
        </button>
        <button
          type="button"
          class="pill-button"
          @click="cycleStatus(message.callsign, 'mobilityStatus')"
        >
          <StatusPill label="Mobility" :value="message.mobilityStatus" />
        </button>
        <button
          type="button"
          class="pill-button"
          @click="cycleStatus(message.callsign, 'commsStatus')"
        >
          <StatusPill label="Comms" :value="message.commsStatus" />
        </button>
      </div>
      <footer class="item-actions">
        <button class="action edit" type="button" @click="emit('edit', message.callsign)">
          Edit
        </button>
        <button class="action delete" type="button" @click="emit('delete', message.callsign)">
          Delete
        </button>
      </footer>
    </article>
  </section>
</template>

<style scoped>
.list {
  display: grid;
  gap: 1rem;
}

.item {
  background:
    linear-gradient(145deg, rgb(18 35 68 / 92%), rgb(10 20 45 / 90%)),
    radial-gradient(circle at 72% 10%, rgb(69 235 255 / 16%), transparent 34%);
  border: 1px solid rgb(90 142 220 / 25%);
  border-radius: 16px;
  padding: 1rem;
}

.item-header {
  display: flex;
  justify-content: space-between;
}

.callsign {
  font-family: var(--font-headline);
  font-size: 1.54rem;
  margin: 0;
}

.group {
  color: #9fb6d8;
  font-family: var(--font-body);
  font-size: 1.3rem;
  font-weight: 600;
  margin: 0;
}

.pills {
  display: flex;
  flex-wrap: wrap;
  margin-top: 0.5rem;
}

.pill-button {
  background: transparent;
  border: 0;
  cursor: pointer;
  padding: 0;
}

.item-actions {
  display: flex;
  gap: 0.8rem;
  justify-content: flex-end;
  margin-top: 0.95rem;
}

.action {
  border-radius: 12px;
  cursor: pointer;
  font-family: var(--font-body);
  font-size: 1.12rem;
  font-weight: 700;
  letter-spacing: 0.01em;
  min-width: 86px;
  padding: 0.45rem 1rem;
}

.edit {
  background: rgb(11 39 84 / 80%);
  border: 1px solid rgb(66 169 255 / 80%);
  box-shadow: 0 0 16px rgb(66 169 255 / 24%);
  color: #61bbff;
}

.delete {
  background: rgb(53 15 25 / 70%);
  border: 1px solid rgb(255 70 91 / 84%);
  box-shadow: 0 0 16px rgb(255 72 104 / 24%);
  color: #ff7b89;
}

@media (max-width: 640px) {
  .callsign {
    font-size: 1.88rem;
  }

  .group {
    font-size: 1.9rem;
    max-width: 44%;
  }
}
</style>
