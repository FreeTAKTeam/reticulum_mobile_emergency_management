<script setup lang="ts">
import logoUrl from "../assets/rem-logo.png";

defineProps<{
  version: string;
}>();
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
  gap: 1.5rem;
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
</style>
