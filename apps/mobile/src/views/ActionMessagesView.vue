<script setup lang="ts">
import { computed, reactive } from "vue";

import ActionMessageList from "../components/ActionMessageList.vue";
import ActionMessageTable from "../components/ActionMessageTable.vue";
import { useMessagesStore } from "../stores/messagesStore";

const messagesStore = useMessagesStore();
messagesStore.init();
messagesStore.initReplication();

const createForm = reactive({
  callsign: "",
  groupName: "Cal team",
});

const messages = computed(() => messagesStore.messages);

async function createMessage(): Promise<void> {
  if (!createForm.callsign.trim()) {
    return;
  }
  await messagesStore.upsertLocal({
    callsign: createForm.callsign.trim(),
    groupName: createForm.groupName.trim() || "Cal team",
    securityStatus: "Unknown",
    capabilityStatus: "Unknown",
    preparednessStatus: "Unknown",
    medicalStatus: "Unknown",
    mobilityStatus: "Unknown",
    commsStatus: "Unknown",
  });
  createForm.callsign = "";
}

function editMessage(callsign: string): void {
  const nextGroup = window.prompt("Update group", "Cal team");
  if (!nextGroup) {
    return;
  }
  const message = messages.value.find((item) => item.callsign === callsign);
  if (!message) {
    return;
  }
  messagesStore
    .upsertLocal({
      ...message,
      groupName: nextGroup.trim() || message.groupName,
    })
    .catch(() => undefined);
}

function deleteMessage(callsign: string): void {
  messagesStore.deleteLocal(callsign).catch(() => undefined);
}
</script>

<template>
  <section class="view">
    <header class="view-header">
      <div>
        <h1>Emergency Action Messages</h1>
        <p>Monitor status updates from field teams and dispatch support.</p>
      </div>
      <span class="badge">{{ messagesStore.activeCount }} active</span>
    </header>

    <form class="create-form" @submit.prevent="createMessage">
      <input
        v-model="createForm.callsign"
        type="text"
        placeholder="New callsign"
        aria-label="New callsign"
      />
      <input
        v-model="createForm.groupName"
        type="text"
        placeholder="Group name"
        aria-label="Group name"
      />
      <button type="submit">Add message</button>
    </form>

    <div class="desktop-only">
      <ActionMessageTable
        :messages="messages"
        @edit="editMessage"
        @delete="deleteMessage"
        @cycle="messagesStore.rotateStatus"
      />
    </div>
    <div class="mobile-only">
      <ActionMessageList
        :messages="messages"
        @edit="editMessage"
        @delete="deleteMessage"
        @cycle="messagesStore.rotateStatus"
      />
    </div>
  </section>
</template>

<style scoped>
.view {
  display: grid;
  gap: 1rem;
}

.view-header {
  align-items: center;
  display: flex;
  justify-content: space-between;
}

h1 {
  font-family: var(--font-headline);
  font-size: clamp(1.9rem, 4vw, 3.1rem);
  margin: 0;
}

p {
  color: #9cb3d6;
  font-family: var(--font-body);
  font-size: clamp(1rem, 1.6vw, 1.3rem);
  margin: 0.2rem 0 0;
}

.badge {
  background: rgb(9 61 108 / 68%);
  border: 1px solid rgb(73 173 255 / 62%);
  border-radius: 999px;
  color: #64beff;
  font-family: var(--font-ui);
  font-size: 0.92rem;
  letter-spacing: 0.08em;
  padding: 0.46rem 0.8rem;
  text-transform: uppercase;
}

.create-form {
  align-items: center;
  display: grid;
  gap: 0.6rem;
  grid-template-columns: minmax(140px, 200px) minmax(160px, 220px) auto;
}

.create-form input {
  background: rgb(8 22 50 / 82%);
  border: 1px solid rgb(75 118 185 / 44%);
  border-radius: 10px;
  color: #d1e9ff;
  font-family: var(--font-body);
  font-size: 1rem;
  padding: 0.5rem 0.6rem;
}

.create-form button {
  background: linear-gradient(110deg, #00a8ff, #14f0ff);
  border: 0;
  border-radius: 11px;
  color: #032748;
  cursor: pointer;
  font-family: var(--font-ui);
  font-size: 0.92rem;
  font-weight: 700;
  letter-spacing: 0.07em;
  min-height: 38px;
  padding: 0 0.9rem;
  text-transform: uppercase;
}

.mobile-only {
  display: none;
}

@media (max-width: 980px) {
  .desktop-only {
    display: none;
  }

  .mobile-only {
    display: block;
  }

  .create-form {
    grid-template-columns: 1fr;
  }
}
</style>
