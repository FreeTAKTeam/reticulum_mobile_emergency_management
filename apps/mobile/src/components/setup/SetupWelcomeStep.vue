<script setup lang="ts">
defineProps<{ logoUrl: string }>();
</script>

<template>
  <section class="wizard-section welcome-section">
    <img class="welcome-symbol" :src="logoUrl" alt="" aria-hidden="true" />
    <p class="eyebrow">Initial setup</p>
    <h1>Reticulum Emergency Manager</h1>
    <p class="intro-copy">Configure REM to get mission ready.</p>
    <div class="mission-list">
      <div class="mission-row">
        <span class="row-icon">ID</span>
        <div><strong>Identity</strong><small>Set call sign and operator identity.</small></div>
      </div>
      <div class="mission-row">
        <span class="row-icon">OK</span>
        <div><strong>Permissions</strong><small>Grant Android permissions required by selected features.</small></div>
      </div>
      <div class="mission-row">
        <span class="row-icon">NET</span>
        <div><strong>Network</strong><small>Configure TCP interfaces for Reticulum access.</small></div>
      </div>
      <div class="mission-row">
        <span class="row-icon">RF</span>
        <div><strong>LoRa</strong><small>Enable RNode Bluetooth LoRa after Bluetooth permission.</small></div>
      </div>
      <div class="mission-row">
        <span class="row-icon">GPS</span>
        <div><strong>Telemetry</strong><small>Choose whether to publish position data.</small></div>
      </div>
      <div class="mission-row danger-row">
        <span class="row-icon">SOS</span>
        <div><strong>SOS</strong><small>Enable emergency messaging and floating SOS access.</small></div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.welcome-section {
  justify-items: center;
  text-align: center;
}

.welcome-symbol {
  filter: drop-shadow(0 0 30px rgb(100 190 255 / 35%));
  margin-top: 0.2rem;
  width: clamp(8rem, 30vw, 15rem);
}

.eyebrow {
  color: #64beff;
  font-family: var(--font-ui);
  font-size: 0.78rem;
  font-weight: 800;
  letter-spacing: 0.12em;
  margin: 0;
  margin-top: 0.2rem;
  text-transform: uppercase;
}

h1 {
  color: #def1ff;
  font-family: var(--font-headline);
  font-size: clamp(2rem, 8vw, 4rem);
  font-weight: 800;
  letter-spacing: 0.04em;
  line-height: 0.95;
  margin: 0;
  text-transform: uppercase;
}

.intro-copy,
small {
  color: #9cb3d6;
  font-family: var(--font-body);
}

.intro-copy {
  margin: 0;
}

.mission-list {
  border: 1px solid rgb(74 133 207 / 42%);
  border-radius: 14px;
  overflow: hidden;
  width: 100%;
}

.mission-row {
  align-items: center;
  background: rgb(5 20 44 / 58%);
  border-bottom: 1px solid rgb(74 133 207 / 32%);
  display: flex;
  gap: 0.7rem;
  padding: 0.72rem 0.78rem;
  text-align: left;
}

.mission-row:last-child {
  border-bottom: 0;
}

.mission-row div {
  display: grid;
  gap: 0.1rem;
}

.mission-row strong {
  color: #def1ff;
  font-family: var(--font-ui);
}

.row-icon {
  align-items: center;
  border: 1px solid rgb(100 190 255 / 62%);
  border-radius: 10px;
  color: #64beff;
  display: inline-flex;
  flex: 0 0 2.65rem;
  font-family: var(--font-ui);
  font-size: 0.68rem;
  font-weight: 900;
  height: 2.65rem;
  justify-content: center;
}

.danger-row .row-icon {
  border-color: rgb(255 102 102 / 80%);
  color: #ff7777;
}

@media (max-width: 760px) {
  .welcome-symbol {
    width: clamp(4.1rem, 22vw, 5.2rem);
  }

  h1 {
    font-size: clamp(1.48rem, 7.4vw, 2.14rem);
  }

  .intro-copy,
  .mission-row small {
    font-size: 0.86rem;
  }

  .intro-copy {
    line-height: 1.25;
  }

  .mission-row {
    gap: 0.54rem;
    padding: 0.26rem 0.48rem;
  }

  .row-icon {
    border-radius: 8px;
    flex-basis: 2.3rem;
    height: 1.95rem;
  }

  .mission-row strong {
    font-size: 0.88rem;
  }

  .mission-row small {
    font-size: 0.78rem;
    line-height: 1.18;
  }
}
</style>
