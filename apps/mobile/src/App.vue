<script setup lang="ts">
import { computed, onMounted } from "vue";
import { RouterLink, RouterView, useRoute } from "vue-router";

import TopServiceCards from "./components/TopServiceCards.vue";
import { useNodeStore } from "./stores/nodeStore";

const nodeStore = useNodeStore();
const route = useRoute();

onMounted(async () => {
  try {
    await nodeStore.init();
    await nodeStore.startNode();
  } catch (error: unknown) {
    nodeStore.lastError = error instanceof Error ? error.message : String(error);
  }
});

const tabItems = [
  { path: "/messages", label: "Action Messages", icon: "MSG" },
  { path: "/events", label: "Events", icon: "EVT" },
  { path: "/dashboard", label: "Dashboard", icon: "DASH" },
  { path: "/settings", label: "Settings", icon: "CFG" },
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

      <TopServiceCards
        :running="nodeStore.status.running"
        :connected-count="nodeStore.connectedDestinations.length"
        :settings="nodeStore.settings"
      />

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
        >
          <span class="icon">{{ tab.icon }}</span>
          <span class="label">{{ tab.label }}</span>
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
  min-height: 100dvh;
  padding: 1rem;
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
  margin: 0 auto;
  max-width: 1600px;
  position: relative;
  z-index: 1;
}

.masthead {
  align-items: center;
  display: flex;
  justify-content: space-between;
  margin-bottom: 0.8rem;
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
  margin-top: 0.95rem;
  min-height: calc(100dvh - 240px);
  padding-bottom: 5rem;
}

.tabs {
  backdrop-filter: blur(9px);
  background: rgb(3 13 33 / 84%);
  border: 1px solid rgb(63 99 157 / 37%);
  border-radius: 13px;
  bottom: 1rem;
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  left: 1rem;
  max-width: calc(1600px - 2rem);
  position: fixed;
  right: 1rem;
}

.tab {
  align-items: center;
  border-right: 1px solid rgb(60 101 160 / 20%);
  color: #8ea5ca;
  display: grid;
  font-family: var(--font-body);
  justify-items: center;
  min-height: 52px;
  padding: 0.3rem;
  text-decoration: none;
}

.tab:last-child {
  border-right: 0;
}

.icon {
  font-size: 1rem;
}

.label {
  font-size: 0.72rem;
}

.tab.active {
  color: #54ceff;
  text-shadow: 0 0 16px rgb(84 206 255 / 40%);
}

@media (max-width: 780px) {
  .app-bg {
    padding: 0.6rem;
  }

  .content {
    min-height: calc(100dvh - 220px);
  }

  .label {
    font-size: 0.64rem;
  }
}
</style>
