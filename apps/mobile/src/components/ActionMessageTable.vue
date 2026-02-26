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
</script>

<template>
  <div class="table-wrap">
    <table class="table">
      <thead>
        <tr>
          <th>Callsign</th>
          <th>Group</th>
          <th>Security</th>
          <th>Capability</th>
          <th>Preparedness</th>
          <th>Medical</th>
          <th>Mobility</th>
          <th>Comms</th>
          <th>Actions</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="message in props.messages" :key="message.callsign">
          <td>{{ message.callsign }}</td>
          <td>{{ message.groupName }}</td>
          <td>
            <button
              class="pill-button"
              type="button"
              @click="emit('cycle', message.callsign, 'securityStatus')"
            >
              <StatusPill label="" :value="message.securityStatus" />
            </button>
          </td>
          <td>
            <button
              class="pill-button"
              type="button"
              @click="emit('cycle', message.callsign, 'capabilityStatus')"
            >
              <StatusPill label="" :value="message.capabilityStatus" />
            </button>
          </td>
          <td>
            <button
              class="pill-button"
              type="button"
              @click="emit('cycle', message.callsign, 'preparednessStatus')"
            >
              <StatusPill label="" :value="message.preparednessStatus" />
            </button>
          </td>
          <td>
            <button
              class="pill-button"
              type="button"
              @click="emit('cycle', message.callsign, 'medicalStatus')"
            >
              <StatusPill label="" :value="message.medicalStatus" />
            </button>
          </td>
          <td>
            <button
              class="pill-button"
              type="button"
              @click="emit('cycle', message.callsign, 'mobilityStatus')"
            >
              <StatusPill label="" :value="message.mobilityStatus" />
            </button>
          </td>
          <td>
            <button
              class="pill-button"
              type="button"
              @click="emit('cycle', message.callsign, 'commsStatus')"
            >
              <StatusPill label="" :value="message.commsStatus" />
            </button>
          </td>
          <td class="actions">
            <button class="action edit" @click="emit('edit', message.callsign)">Edit</button>
            <button class="action delete" @click="emit('delete', message.callsign)">Delete</button>
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<style scoped>
.table-wrap {
  overflow-x: auto;
}

.table {
  border-collapse: collapse;
  min-width: 100%;
  width: 100%;
}

.table thead th {
  color: #8ea8d1;
  font-family: var(--font-ui);
  font-size: 0.82rem;
  letter-spacing: 0.08em;
  padding: 0.6rem 0.7rem;
  text-align: left;
  text-transform: uppercase;
}

.table tbody td {
  border-top: 1px solid rgb(82 128 190 / 20%);
  color: #d8ecff;
  font-family: var(--font-body);
  font-size: 1.02rem;
  padding: 0.55rem 0.7rem;
}

.pill-button {
  background: transparent;
  border: 0;
  cursor: pointer;
  padding: 0;
}

.actions {
  display: flex;
  gap: 0.4rem;
}

.action {
  border-radius: 11px;
  cursor: pointer;
  font-family: var(--font-body);
  font-size: 0.96rem;
  font-weight: 700;
  padding: 0.35rem 0.72rem;
}

.edit {
  background: rgb(11 39 84 / 80%);
  border: 1px solid rgb(66 169 255 / 80%);
  color: #61bbff;
}

.delete {
  background: rgb(53 15 25 / 70%);
  border: 1px solid rgb(255 70 91 / 84%);
  color: #ff7b89;
}
</style>
