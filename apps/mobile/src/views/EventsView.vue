<script setup lang="ts">
import { reactive } from "vue";

import { useEventsStore } from "../stores/eventsStore";

const eventsStore = useEventsStore();
eventsStore.init();
eventsStore.initReplication();

const createForm = reactive({
  callsign: "",
  type: "Incident",
  summary: "",
});

async function createEvent(): Promise<void> {
  if (!createForm.callsign.trim() || !createForm.summary.trim()) {
    return;
  }
  await eventsStore.upsertLocal({
    callsign: createForm.callsign.trim(),
    type: createForm.type.trim() || "Incident",
    summary: createForm.summary.trim(),
  });
  createForm.summary = "";
}

async function deleteEvent(uid: string): Promise<void> {
  await eventsStore.deleteLocal(uid);
}
</script>

<template>
  <section class="view">
    <header class="view-header">
      <h1>Events</h1>
      <p>Live replicated incident feed across connected peers.</p>
    </header>

    <form class="create-form" @submit.prevent="createEvent">
      <input
        v-model="createForm.callsign"
        type="text"
        placeholder="Callsign"
        aria-label="Callsign"
      />
      <input
        v-model="createForm.type"
        type="text"
        placeholder="Type"
        aria-label="Type"
      />
      <input
        v-model="createForm.summary"
        type="text"
        placeholder="Event summary"
        aria-label="Event summary"
      />
      <button type="submit">Add event</button>
    </form>

    <section class="timeline">
      <article class="event" v-for="event in eventsStore.records" :key="event.uid">
        <div class="event-head">
          <p class="event-type">{{ event.type }}</p>
          <button type="button" @click="deleteEvent(event.uid)">Delete</button>
        </div>
        <h3>{{ event.summary }}</h3>
        <p class="meta">
          {{ event.callsign }} | {{ new Date(event.updatedAt).toLocaleTimeString() }}
        </p>
      </article>
      <p v-if="eventsStore.records.length === 0" class="empty">
        No events yet. Add one locally or wait for a peer snapshot.
      </p>
    </section>
  </section>
</template>

<style scoped>
.view {
  display: grid;
  gap: 1rem;
}

h1 {
  font-family: var(--font-headline);
  font-size: clamp(1.8rem, 3.6vw, 2.9rem);
  margin: 0;
}

.view-header p {
  color: #9cb3d6;
  font-family: var(--font-body);
  font-size: clamp(1rem, 1.6vw, 1.25rem);
  margin: 0.25rem 0 0;
}

.create-form {
  align-items: center;
  display: grid;
  gap: 0.6rem;
  grid-template-columns: minmax(120px, 170px) minmax(120px, 160px) 1fr auto;
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
  font-size: 0.85rem;
  font-weight: 700;
  letter-spacing: 0.07em;
  min-height: 38px;
  padding: 0 0.9rem;
  text-transform: uppercase;
}

.timeline {
  display: grid;
  gap: 0.8rem;
}

.event {
  background:
    radial-gradient(circle at 18% 20%, rgb(33 115 255 / 17%), transparent 46%),
    linear-gradient(130deg, rgb(13 32 65 / 92%), rgb(9 19 43 / 90%));
  border: 1px solid rgb(73 112 170 / 28%);
  border-radius: 14px;
  padding: 0.8rem 1rem;
}

.event-head {
  align-items: center;
  display: flex;
  justify-content: space-between;
}

.event-type {
  color: #74beff;
  font-family: var(--font-ui);
  font-size: 0.76rem;
  font-weight: 700;
  letter-spacing: 0.13em;
  margin: 0;
  text-transform: uppercase;
}

h3 {
  font-family: var(--font-body);
  font-size: 1.06rem;
  margin: 0.26rem 0 0;
}

.meta {
  color: #8da7cd;
  font-family: var(--font-body);
  margin: 0.3rem 0 0;
}

.event button {
  background: rgb(84 14 42 / 73%);
  border: 1px solid rgb(255 86 120 / 72%);
  border-radius: 9px;
  color: #ff95b0;
  cursor: pointer;
  font-family: var(--font-ui);
  font-size: 0.72rem;
  letter-spacing: 0.09em;
  min-height: 28px;
  padding: 0 0.58rem;
  text-transform: uppercase;
}

.empty {
  color: #8da7cd;
  font-family: var(--font-body);
  margin: 0;
}

@media (max-width: 920px) {
  .create-form {
    grid-template-columns: 1fr;
  }
}
</style>
