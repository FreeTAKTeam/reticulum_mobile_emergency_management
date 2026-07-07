<script setup lang="ts">
import logoUrl from "../assets/rem-logo.png";

type InterfaceLoadingState = "disabled" | "loading" | "waiting" | "ready";

interface InterfaceLoadingItem {
  id: string;
  label: string;
  detail: string;
  state: InterfaceLoadingState;
}

withDefaults(defineProps<{
  version: string;
  interfaceLoading?: boolean;
  interfaceItems?: InterfaceLoadingItem[];
  loadingMessage: string;
  loadingDetail: string;
}>(), {
  interfaceLoading: false,
  interfaceItems: () => [],
});
</script>

<template>
  <section
    class="splash-screen"
    data-testid="splash-screen"
    aria-label="R.E.M. startup"
  >
    <div class="splash-mark-ring">
      <img class="splash-logo" :src="logoUrl" alt="R.E.M. logo" />
    </div>
    <div class="splash-copy">
      <p class="splash-name">R.E.M.</p>
      <p class="splash-title">Reticulum Mobile Emergency Management</p>
      <p class="splash-version">Version {{ version }}</p>
    </div>
    <div
      v-if="interfaceLoading"
      class="splash-interface-loading"
      data-testid="splash-interface-loading"
      role="status"
      aria-live="polite"
    >
      <div class="loading-copy">
        <p class="loading-kicker">{{ loadingMessage }}</p>
        <span
          class="loading-progress"
          data-testid="splash-loading-animation"
          aria-hidden="true"
        ></span>
        <p class="loading-detail">{{ loadingDetail }}</p>
      </div>
      <ul class="interface-list" aria-label="Interface startup status">
        <li
          v-for="item in interfaceItems"
          :key="item.id"
          class="interface-row"
          :class="`interface-row-${item.state}`"
          :data-testid="`splash-interface-${item.id}`"
        >
          <span class="interface-indicator" aria-hidden="true"></span>
          <span class="interface-copy">
            <span class="interface-label">{{ item.label }}</span>
            <span class="interface-detail">{{ item.detail }}</span>
          </span>
          <span class="interface-state">{{ item.state }}</span>
        </li>
      </ul>
    </div>
  </section>
</template>

<style scoped>
.splash-screen {
  align-items: center;
  background:
    linear-gradient(135deg, rgb(4 14 33 / 96%), rgb(2 31 55 / 96%)),
    radial-gradient(circle at 50% 28%, rgb(42 214 255 / 18%), transparent 34%);
  color: #f4fbff;
  display: grid;
  gap: 1.2rem;
  inset: 0;
  justify-items: center;
  padding: 2rem;
  position: fixed;
  text-align: center;
  z-index: 50;
}

.splash-screen::before {
  background-image:
    linear-gradient(rgb(73 126 192 / 12%) 1px, transparent 1px),
    linear-gradient(90deg, rgb(73 126 192 / 12%) 1px, transparent 1px);
  background-size: 68px 68px;
  content: "";
  inset: 0;
  mask-image: radial-gradient(circle at center, black, transparent 72%);
  opacity: 0.7;
  position: absolute;
}

.splash-mark-ring,
.splash-copy {
  position: relative;
}

.splash-mark-ring {
  align-items: center;
  aspect-ratio: 1;
  background:
    linear-gradient(145deg, rgb(26 78 119 / 84%), rgb(4 17 38 / 88%)),
    rgb(6 20 43);
  border: 1px solid rgb(78 215 247 / 65%);
  border-radius: 24px;
  box-shadow:
    0 0 0 10px rgb(40 118 171 / 18%),
    0 24px 68px rgb(0 11 28 / 70%),
    inset 0 0 36px rgb(46 222 255 / 16%);
  display: grid;
  justify-items: center;
  width: min(42vw, 9.5rem);
}

.splash-logo {
  display: block;
  filter: drop-shadow(0 0 18px rgb(74 221 255 / 54%));
  width: 76%;
}

.splash-copy {
  display: grid;
  gap: 0.35rem;
}

.splash-name,
.splash-title,
.splash-version {
  margin: 0;
}

.splash-name {
  color: #ffffff;
  font-family: var(--font-headline);
  font-size: 2.8rem;
  letter-spacing: 0;
  line-height: 1;
}

.splash-title {
  color: #a9d8ff;
  font-family: var(--font-ui);
  font-size: 0.94rem;
  font-weight: 700;
  text-transform: uppercase;
}

.splash-version {
  border-top: 1px solid rgb(95 183 234 / 32%);
  color: #5ee7ff;
  font-family: var(--font-ui);
  font-size: 0.86rem;
  font-weight: 700;
  margin-top: 0.55rem;
  padding-top: 0.7rem;
}

.splash-interface-loading {
  background: rgb(2 15 34 / 72%);
  border: 1px solid rgb(95 183 234 / 32%);
  border-radius: 8px;
  box-shadow:
    0 18px 44px rgb(0 8 22 / 42%),
    inset 0 0 28px rgb(73 207 255 / 8%);
  display: grid;
  gap: 0.95rem;
  max-width: min(90vw, 24rem);
  padding: 1rem;
  position: relative;
  width: 100%;
}

.loading-copy {
  display: grid;
  gap: 0.42rem;
}

.loading-kicker,
.loading-detail {
  margin: 0;
}

.loading-kicker {
  color: #f4fbff;
  font-family: var(--font-ui);
  font-size: 0.84rem;
  font-weight: 800;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.loading-detail {
  color: #a9d8ff;
  font-family: var(--font-ui);
  font-size: 0.78rem;
  font-weight: 650;
}

.loading-progress {
  background: rgb(30 86 130 / 72%);
  border-radius: 999px;
  display: block;
  height: 0.28rem;
  overflow: hidden;
  position: relative;
  width: 100%;
}

.loading-progress::before {
  animation: loading-scan 1.2s ease-in-out infinite;
  background: linear-gradient(90deg, transparent, #5ee7ff 50%, transparent);
  content: "";
  height: 100%;
  left: -45%;
  position: absolute;
  top: 0;
  width: 45%;
}

.interface-list {
  display: grid;
  gap: 0.5rem;
  list-style: none;
  margin: 0;
  padding: 0;
}

.interface-row {
  align-items: center;
  background: rgb(8 34 65 / 68%);
  border: 1px solid rgb(111 201 247 / 18%);
  border-radius: 7px;
  display: grid;
  gap: 0.68rem;
  grid-template-columns: 0.62rem minmax(0, 1fr) auto;
  min-height: 3.1rem;
  padding: 0.58rem 0.7rem;
  text-align: left;
}

.interface-indicator {
  aspect-ratio: 1;
  background: #4bdcff;
  border-radius: 50%;
  box-shadow: 0 0 0 0 rgb(75 220 255 / 50%);
  width: 0.62rem;
}

.interface-row-loading .interface-indicator,
.interface-row-waiting .interface-indicator {
  animation: interface-pulse 1.35s ease-out infinite;
}

.interface-row-loading .interface-indicator {
  background: #ff5d6c;
  box-shadow: 0 0 0 0 rgb(255 93 108 / 50%);
}

.interface-row-waiting .interface-indicator {
  background: #ffcf5d;
}

.interface-row-ready .interface-indicator {
  background: #5df2a0;
  box-shadow: 0 0 14px rgb(93 242 160 / 42%);
}

.interface-row-disabled {
  opacity: 0.78;
}

.interface-row-disabled .interface-indicator {
  background: #7b8794;
  box-shadow: none;
}

.interface-copy {
  display: grid;
  gap: 0.1rem;
  min-width: 0;
}

.interface-label {
  color: #ffffff;
  font-family: var(--font-ui);
  font-size: 0.84rem;
  font-weight: 800;
  line-height: 1.15;
}

.interface-detail {
  color: #a9d8ff;
  font-family: var(--font-ui);
  font-size: 0.74rem;
  font-weight: 650;
  line-height: 1.25;
  overflow-wrap: anywhere;
}

.interface-state {
  background: rgb(16 58 92 / 72%);
  border: 1px solid rgb(94 231 255 / 28%);
  border-radius: 999px;
  color: #5ee7ff;
  font-family: var(--font-ui);
  font-size: 0.68rem;
  font-weight: 850;
  min-width: 4.8rem;
  padding: 0.28rem 0.5rem;
  text-align: center;
  text-transform: uppercase;
}

.interface-row-disabled .interface-state {
  background: #3f4852;
  border-color: #687380;
  color: #d2d8df;
}

@keyframes interface-pulse {
  0% {
    box-shadow: 0 0 0 0 rgb(75 220 255 / 48%);
  }

  72% {
    box-shadow: 0 0 0 0.42rem rgb(75 220 255 / 0%);
  }

  100% {
    box-shadow: 0 0 0 0 rgb(75 220 255 / 0%);
  }
}

@keyframes loading-scan {
  0% {
    transform: translateX(0);
  }

  100% {
    transform: translateX(320%);
  }
}

@media (max-width: 390px) {
  .splash-screen {
    gap: 0.95rem;
    padding: 1.2rem;
  }

  .splash-mark-ring {
    width: min(40vw, 7.4rem);
  }

  .splash-name {
    font-size: 2.35rem;
  }

  .splash-interface-loading {
    padding: 0.82rem;
  }
}
</style>
