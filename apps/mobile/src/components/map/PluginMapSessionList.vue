<script setup lang="ts">
import { computed, onMounted } from "vue";

import { useMapMarkersStore } from "../../stores/mapMarkersStore";
import { usePluginMapSessionsStore } from "../../stores/pluginMapSessionsStore";

const props = withDefaults(defineProps<{ pluginId?: string; compact?: boolean }>(), {
  pluginId: undefined,
  compact: false,
});

const sessionsStore = usePluginMapSessionsStore();
const markersStore = useMapMarkersStore();

const visibleSessions = computed(() =>
  sessionsStore.sessions.filter((session) => !props.pluginId || session.pluginId === props.pluginId),
);

function objectLabel(objectDestinationHashes: string[]): string {
  const names = objectDestinationHashes.map((key) =>
    markersStore.markers.find((marker) => marker.objectDestinationHash === key)?.name ?? key.slice(0, 8));
  return names.join(", ");
}

onMounted(() => {
  void sessionsStore.init();
});
</script>

<template>
  <section
    v-if="visibleSessions.length"
    class="plugin-map-sessions"
    :class="{ compact }"
    aria-label="Active plugin map sessions"
    data-testid="plugin-map-sessions"
  >
    <article v-for="session in visibleSessions" :key="session.sessionId" class="plugin-map-session">
      <p>
        <strong>{{ sessionsStore.pluginNames.get(session.pluginId) ?? session.pluginId }}</strong>
        can read {{ objectLabel(session.objectDestinationHashes) }}
      </p>
      <button
        type="button"
        class="plugin-map-session-stop"
        :disabled="sessionsStore.busy"
        :aria-label="`Stop map session for ${sessionsStore.pluginNames.get(session.pluginId) ?? session.pluginId}`"
        @click="sessionsStore.stop(session.sessionId)"
      >
        Stop
      </button>
    </article>
    <p v-if="sessionsStore.lastError" class="plugin-map-session-error" role="alert">
      {{ sessionsStore.lastError }}
    </p>
  </section>
</template>

<style scoped>
.plugin-map-sessions {
  display: grid;
  gap: 0.4rem;
}

.plugin-map-sessions.compact {
  background: rgb(8 18 34 / 88%);
  border: 1px solid rgb(100 190 255 / 35%);
  border-radius: 0.8rem;
  padding: 0.5rem 0.7rem;
}

.plugin-map-session {
  align-items: center;
  display: flex;
  gap: 0.6rem;
  justify-content: space-between;
}

.plugin-map-session p {
  color: #def1ff;
  font-size: 0.86rem;
  margin: 0;
  overflow-wrap: anywhere;
}

.plugin-map-session-error {
  color: #ff9c9c;
  font-size: 0.84rem;
  margin: 0;
}
</style>
