<script setup lang="ts">
import { computed, onMounted } from "vue";
import { RouterLink, RouterView, useRoute } from "vue-router";

import { initAppNotifications } from "./services/notifications";
import { useEventsStore } from "./stores/eventsStore";
import { useMessagesStore } from "./stores/messagesStore";
import { useNodeStore } from "./stores/nodeStore";

const nodeStore = useNodeStore();
const messagesStore = useMessagesStore();
const eventsStore = useEventsStore();
const route = useRoute();

onMounted(async () => {
  try {
    await initAppNotifications();
    messagesStore.init();
    messagesStore.initReplication();
    eventsStore.init();
    eventsStore.initReplication();

    await nodeStore.init();
    await nodeStore.startNode();
  } catch (error: unknown) {
    nodeStore.lastError = error instanceof Error ? error.message : String(error);
  }
});

const tabItems = [
  { path: "/messages", label: "Action Messages", icon: "messages" },
  { path: "/events", label: "Events", icon: "events" },
  { path: "/dashboard", label: "Dashboard", icon: "dashboard" },
  { path: "/settings", label: "Settings", icon: "settings" },
];

const runningText = computed(() => (nodeStore.status.running ? "Active" : "Offline"));
</script>

<template>
  <div class="app-bg">
    <div class="app-shell">
      <header class="masthead">
        <div class="brand">
          <p class="asterisk">*</p>
          <div>
            <p class="title">Emergency Ops</p>
            <p class="subtitle">Rapid Mesh Response</p>
          </div>
        </div>
        <div class="mast-actions">
          <span class="running">{{ runningText }}</span>
          <RouterLink to="/peers" class="peers-link">Peers &amp; Discovery</RouterLink>
        </div>
      </header>

      <main class="content">
        <RouterView />
      </main>

      <nav class="tabs">
        <RouterLink
          v-for="tab in tabItems"
          :key="tab.path"
          :to="tab.path"
          class="tab"
          :class="{ active: route.path === tab.path }"
          :aria-label="tab.label"
          :title="tab.label"
        >
          <span class="tab-icon" aria-hidden="true">
            <svg
              v-if="tab.icon === 'messages'"
              class="icon-svg"
              viewBox="0 0 24 24"
              fill="none"
            >
              <path
                d="M6 7.5h12a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2H11l-4 3v-3H6a2 2 0 0 1-2-2v-6a2 2 0 0 1 2-2Z"
              />
              <path d="M8 11h8" />
              <path d="M8 14h5" />
            </svg>
            <svg
              v-else-if="tab.icon === 'events'"
              class="icon-svg"
              viewBox="0 0 24 24"
              fill="none"
            >
              <path
                d="M12 20.5s5-4.7 5-9.1a5 5 0 1 0-10 0c0 4.4 5 9.1 5 9.1Z"
              />
              <path d="M12 13.2a1.9 1.9 0 1 0 0-3.8 1.9 1.9 0 0 0 0 3.8Z" />
            </svg>
            <svg
              v-else-if="tab.icon === 'dashboard'"
              class="icon-svg"
              viewBox="0 0 24 24"
              fill="none"
            >
              <path d="M5 5h5v5H5z" />
              <path d="M14 5h5v8h-5z" />
              <path d="M5 14h5v5H5z" />
              <path d="M14 16h5v3h-5z" />
            </svg>
            <svg
              v-else
              class="icon-svg"
              viewBox="0 0 24 24"
              fill="none"
            >
              <path d="M5 7h10" />
              <path d="M5 17h14" />
              <path d="M15 7a2 2 0 1 0 0-4 2 2 0 0 0 0 4Z" transform="translate(0 2)" />
              <path d="M9 17a2 2 0 1 0 0-4 2 2 0 0 0 0 4Z" transform="translate(0 2)" />
            </svg>
          </span>
          <span class="sr-only">{{ tab.label }}</span>
        </RouterLink>
      </nav>
    </div>
  </div>
</template>

<style scoped>
.app-bg {
  background:
    radial-gradient(circle at 78% -15%, rgb(35 124 255 / 34%), transparent 42%),
    radial-gradient(circle at -5% 100%, rgb(10 164 255 / 20%), transparent 38%),
    linear-gradient(170deg, #030914, #091632 44%, #06142f 100%);
  height: 100dvh;
  min-height: 100dvh;
  overflow: hidden;
  padding-bottom: calc(env(safe-area-inset-bottom, 0px) + 0.8rem);
  padding-inline: 0.8rem;
  padding-top: calc(env(safe-area-inset-top, 0px) + 0.9rem);
}

.app-bg::before {
  background-image:
    linear-gradient(rgb(34 69 115 / 22%) 1px, transparent 1px),
    linear-gradient(90deg, rgb(34 69 115 / 22%) 1px, transparent 1px);
  background-size: 26px 26px;
  content: "";
  inset: 0;
  opacity: 0.5;
  pointer-events: none;
  position: fixed;
}

.app-shell {
  display: grid;
  gap: 0.95rem;
  grid-template-rows: auto minmax(0, 1fr) auto;
  height: 100%;
  margin: 0 auto;
  max-width: 1600px;
  min-height: 0;
  position: relative;
  z-index: 1;
}

.masthead {
  align-items: center;
  display: flex;
  justify-content: space-between;
}

.brand {
  align-items: center;
  display: flex;
  gap: 0.7rem;
}

.asterisk {
  color: #00d4ff;
  font-family: var(--font-headline);
  font-size: 2.8rem;
  line-height: 0.8;
  margin: 0;
  text-shadow: 0 0 20px rgb(0 212 255 / 48%);
}

.title {
  font-family: var(--font-headline);
  font-size: clamp(1.2rem, 2vw, 1.8rem);
  margin: 0;
  text-transform: uppercase;
}

.subtitle {
  color: #7da0d7;
  font-family: var(--font-ui);
  font-size: 0.72rem;
  letter-spacing: 0.12em;
  margin: 0.1rem 0 0;
  text-transform: uppercase;
}

.mast-actions {
  align-items: center;
  display: flex;
  gap: 0.65rem;
}

.running {
  background: rgb(7 47 84 / 74%);
  border: 1px solid rgb(73 171 255 / 54%);
  border-radius: 999px;
  color: #6ac1ff;
  font-family: var(--font-ui);
  font-size: 0.74rem;
  letter-spacing: 0.09em;
  padding: 0.3rem 0.62rem;
  text-transform: uppercase;
}

.peers-link {
  background: linear-gradient(115deg, #0ca0ff, #16edff);
  border-radius: 11px;
  color: #05274b;
  font-family: var(--font-ui);
  font-size: 0.78rem;
  font-weight: 700;
  letter-spacing: 0.09em;
  padding: 0.48rem 0.74rem;
  text-decoration: none;
  text-transform: uppercase;
}

.content {
  -webkit-overflow-scrolling: touch;
  min-height: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding-bottom: 0.2rem;
  padding-right: 0.15rem;
  scrollbar-gutter: stable both-edges;
}

.tabs {
  align-self: end;
  backdrop-filter: blur(9px);
  background: rgb(3 13 33 / 84%);
  border: 1px solid rgb(63 99 157 / 37%);
  border-radius: 13px;
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  max-width: 100%;
}

.tab {
  align-items: center;
  border-right: 1px solid rgb(60 101 160 / 20%);
  color: #8ea5ca;
  display: grid;
  justify-items: center;
  min-height: 50px;
  padding: 0.35rem;
  position: relative;
  text-decoration: none;
}

.tab:last-child {
  border-right: 0;
}

.tab-icon {
  align-items: center;
  display: inline-flex;
  height: 1.45rem;
  justify-content: center;
  width: 1.45rem;
}

.icon-svg {
  height: 100%;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.7;
  width: 100%;
}

.tab.active {
  color: #54ceff;
  text-shadow: 0 0 16px rgb(84 206 255 / 40%);
}

.sr-only {
  border: 0;
  clip: rect(0 0 0 0);
  height: 1px;
  margin: -1px;
  overflow: hidden;
  padding: 0;
  position: absolute;
  white-space: nowrap;
  width: 1px;
}

@media (max-width: 780px) {
  .app-bg {
    padding-bottom: calc(env(safe-area-inset-bottom, 0px) + 0.6rem);
    padding-inline: 0.6rem;
    padding-top: calc(env(safe-area-inset-top, 0px) + 0.6rem);
  }
}
</style>
