<script setup lang="ts">
import logoUrl from "../assets/rem-logo.png";

const statusItems = [
  {
    label: "Pairing",
    value: "Inactive",
  },
  {
    label: "Sensor stream",
    value: "Disabled",
  },
  {
    label: "Data sync",
    value: "Not configured",
  },
] as const;
</script>

<template>
  <section class="bio-view">
    <div class="bio-console">
      <header class="bio-topbar">
        <div class="brand-lockup">
          <img class="brand-symbol" :src="logoUrl" alt="Reticulum Emergency Manager logo" />
          <div class="brand-copy">
            <p>Reticulum</p>
            <span>Emergency Manager</span>
          </div>
        </div>
        <span class="inactive-chip">Inactive</span>
      </header>

      <section class="bio-header">
        <p class="eyebrow">Future module</p>
        <h1>Bio Sensors</h1>
        <p>
          Reserved for future sensor integration.
        </p>
      </section>

      <div class="inactive-panel">
        <div class="sensor-glyph" aria-hidden="true">
          <span></span>
          <span></span>
          <span></span>
        </div>
        <div class="panel-copy">
          <h2>Bio Sensors are inactive</h2>
          <p>
            No sensor pairing, Bluetooth scanning, or native bio-sensor integration is exposed in
            this version.
          </p>
        </div>
      </div>

      <dl class="status-grid">
        <div v-for="item in statusItems" :key="item.label" class="status-item">
          <dt>{{ item.label }}</dt>
          <dd>{{ item.value }}</dd>
        </div>
      </dl>

      <div class="availability-row">
        <span>Available after feature activation</span>
        <strong>Settings</strong>
      </div>
    </div>
  </section>
</template>

<style scoped>
.bio-view {
  --bio-border: rgb(74 133 207 / 45%);
  color: #def1ff;
  display: grid;
  min-height: 100%;
}

.bio-console {
  background:
    radial-gradient(circle at 50% 18%, rgb(100 190 255 / 10%), transparent 34%),
    linear-gradient(180deg, rgb(3 13 30 / 96%), #020710 84%);
  border: 1px solid rgb(64 190 255 / 34%);
  border-radius: 16px;
  box-shadow:
    inset 0 0 0 1px rgb(9 42 76 / 72%),
    0 18px 52px rgb(0 0 0 / 32%);
  display: grid;
  gap: 0.86rem;
  padding: clamp(0.82rem, 2.2vw, 1.2rem);
}

.bio-topbar,
.brand-lockup,
.inactive-panel,
.availability-row {
  align-items: center;
  display: flex;
}

.bio-topbar {
  justify-content: space-between;
}

.brand-lockup {
  gap: 0.72rem;
  min-width: 0;
}

.brand-symbol {
  filter: drop-shadow(0 0 14px rgb(100 190 255 / 28%));
  height: clamp(3rem, 9vw, 4rem);
  width: clamp(3rem, 9vw, 4rem);
}

.brand-copy {
  display: grid;
  text-transform: uppercase;
}

.brand-copy p,
.brand-copy span,
.eyebrow,
.inactive-chip,
.status-item dt,
.availability-row strong {
  font-family: var(--font-ui);
}

.brand-copy p {
  color: #def1ff;
  font-size: clamp(1.45rem, 4vw, 2.4rem);
  font-weight: 800;
  letter-spacing: 0.15em;
  line-height: 0.88;
  margin: 0;
}

.brand-copy span {
  color: #9ee2ff;
  font-size: clamp(0.78rem, 2vw, 1.1rem);
  font-weight: 700;
  letter-spacing: 0.1em;
}

.inactive-chip {
  border: 1px solid rgb(255 180 171 / 54%);
  border-radius: 999px;
  color: #ffb4ab;
  font-size: 0.7rem;
  font-weight: 900;
  letter-spacing: 0.12em;
  padding: 0.42rem 0.64rem;
  text-transform: uppercase;
}

.bio-header,
.inactive-panel,
.status-item,
.availability-row {
  background:
    linear-gradient(150deg, rgb(9 25 55 / 90%), rgb(7 16 37 / 94%)),
    radial-gradient(circle at 8% 0%, rgb(54 193 255 / 11%), transparent 36%);
  border: 1px solid var(--bio-border);
  border-radius: 14px;
}

.bio-header {
  display: grid;
  gap: 0.32rem;
  padding: 0.95rem;
}

.eyebrow {
  color: #64beff;
  font-size: 0.72rem;
  font-weight: 900;
  letter-spacing: 0.12em;
  margin: 0;
  text-transform: uppercase;
}

.bio-header h1,
.panel-copy h2 {
  color: #def1ff;
  font-family: var(--font-headline);
  font-weight: 800;
  letter-spacing: 0.03em;
  line-height: 1;
  margin: 0;
  text-transform: uppercase;
}

.bio-header h1 {
  font-size: clamp(2rem, 6vw, 3.4rem);
}

.bio-header p,
.panel-copy p,
.status-item dd,
.availability-row span {
  color: #9cb3d6;
  font-family: var(--font-body);
  margin: 0;
}

.inactive-panel {
  gap: 1rem;
  padding: 1rem;
}

.sensor-glyph {
  align-items: center;
  aspect-ratio: 1;
  background: rgb(2 7 16 / 58%);
  border: 1px solid rgb(100 190 255 / 42%);
  border-radius: 14px;
  display: grid;
  flex: 0 0 5.5rem;
  justify-items: center;
  padding: 0.8rem;
}

.sensor-glyph span {
  background: #64beff;
  border-radius: 999px;
  box-shadow: 0 0 18px rgb(100 190 255 / 38%);
  display: block;
  height: 0.42rem;
}

.sensor-glyph span:nth-child(1) {
  width: 2.4rem;
}

.sensor-glyph span:nth-child(2) {
  width: 3.4rem;
}

.sensor-glyph span:nth-child(3) {
  width: 1.7rem;
}

.panel-copy {
  display: grid;
  gap: 0.36rem;
}

.status-grid {
  display: grid;
  gap: 0.62rem;
  grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
  margin: 0;
}

.status-item {
  display: grid;
  gap: 0.2rem;
  padding: 0.76rem;
}

.status-item dt {
  color: #64beff;
  font-size: 0.72rem;
  font-weight: 900;
  letter-spacing: 0.1em;
  text-transform: uppercase;
}

.availability-row {
  justify-content: space-between;
  padding: 0.74rem 0.82rem;
}

.availability-row strong {
  color: #64beff;
  font-size: 0.72rem;
  letter-spacing: 0.1em;
  text-transform: uppercase;
}

@media (max-width: 680px) {
  .bio-console {
    padding: 0.72rem;
  }

  .brand-symbol {
    height: 3rem;
    width: 3rem;
  }

  .brand-copy p {
    font-size: 1.42rem;
  }

  .brand-copy span {
    font-size: 0.72rem;
  }

  .inactive-chip {
    display: none;
  }

  .inactive-panel {
    align-items: stretch;
    flex-direction: column;
  }

  .sensor-glyph {
    flex-basis: auto;
    width: 5.2rem;
  }
}
</style>
