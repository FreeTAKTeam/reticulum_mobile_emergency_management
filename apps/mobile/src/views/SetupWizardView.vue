<script setup lang="ts">
import { onMounted } from "vue";

import WizardProgress from "../components/setup/WizardProgress.vue";
import { useSetupWizard } from "../composables/useSetupWizard";
import logoUrl from "../assets/rem-logo.png";
import { TCP_COMMUNITY_SERVERS, toTcpEndpoint } from "../utils/tcpCommunityServers";

const wizard = useSetupWizard();

const tcpServerOptions = TCP_COMMUNITY_SERVERS.map((server) => ({
  name: server.name,
  endpoint: toTcpEndpoint(server),
  isBootstrap: Boolean(server.isBootstrap),
}));

onMounted(() => {
  wizard.open();
});
</script>

<template>
  <section class="setup-view" data-testid="setup-wizard">
    <div class="setup-console">
      <header class="console-header">
        <div class="brand-lockup">
          <img class="brand-symbol" :src="logoUrl" alt="Reticulum Emergency Manager logo" />
          <div class="brand-copy">
            <p>Reticulum</p>
            <span>Emergency Manager</span>
          </div>
        </div>
        <div class="node-state" aria-label="Node readiness preview">
          <span class="state-dot"></span>
          <span>Node<br /><strong>Ready</strong></span>
        </div>
      </header>

      <div class="step-band">
        <div>
          <p class="band-label">Setup Wizard</p>
          <span class="band-underline"></span>
        </div>
        <span class="band-step">Step {{ wizard.activeIndex.value + 1 }} of {{ wizard.steps.length }}</span>
      </div>

      <WizardProgress :steps="wizard.steps" :active-index="wizard.activeIndex.value" />

      <article class="wizard-panel" :class="`step-${wizard.activeStep.value.id}`">
        <section v-if="wizard.activeStep.value.id === 'welcome'" class="wizard-section welcome-section">
          <img class="welcome-symbol" :src="logoUrl" alt="" aria-hidden="true" />
          <p class="eyebrow">Initial setup</p>
          <h1>Reticulum Emergency Manager</h1>
          <p class="intro-copy">
            Configure REM to get mission ready.
          </p>
          <div class="mission-list">
            <div class="mission-row">
              <span class="row-icon">ID</span>
              <div>
                <strong>Identity</strong>
                <small>Set call sign and operator identity.</small>
              </div>
            </div>
            <div class="mission-row">
              <span class="row-icon">NET</span>
              <div>
                <strong>Network</strong>
                <small>Configure TCP interfaces for Reticulum access.</small>
              </div>
            </div>
            <div class="mission-row">
              <span class="row-icon">GPS</span>
              <div>
                <strong>Telemetry</strong>
                <small>Choose whether to publish position data.</small>
              </div>
            </div>
            <div class="mission-row danger-row">
              <span class="row-icon">SOS</span>
              <div>
                <strong>SOS</strong>
                <small>Enable emergency messaging and floating SOS access.</small>
              </div>
            </div>
            <div class="mission-row">
              <span class="row-icon">OK</span>
              <div>
                <strong>Permissions</strong>
                <small>Grant Android permissions required by features.</small>
              </div>
            </div>
          </div>
        </section>

        <section v-else-if="wizard.activeStep.value.id === 'callsign'" class="wizard-section">
          <div class="section-heading">
            <p class="eyebrow">Operator identity</p>
            <h1>Set Call Sign</h1>
            <p>Used for messages, telemetry, and node announcements.</p>
          </div>
          <label class="field-block">
            <span>Call Sign</span>
            <input
              v-model="wizard.draft.displayName"
              type="text"
              maxlength="64"
              autocomplete="off"
              data-testid="setup-callsign"
            />
          </label>
          <div class="status-strip" :class="{ blocked: !wizard.normalizedDisplayName.value }">
            <span>{{ wizard.normalizedDisplayName.value ? "Identity ready" : "Call sign required" }}</span>
            <strong>{{ wizard.normalizedDisplayName.value || "Required" }}</strong>
          </div>
          <div class="preview-panel">
            <span>Announced as</span>
            <strong>{{ wizard.normalizedDisplayName.value || "Unset" }}</strong>
            <small>Node identity will populate after runtime startup.</small>
          </div>
        </section>

        <section v-else-if="wizard.activeStep.value.id === 'tcp'" class="wizard-section">
          <div class="section-heading">
            <p class="eyebrow">Reticulum reachability</p>
            <h1>TCP Interfaces</h1>
            <p>Select known access points or add a custom host:port endpoint.</p>
          </div>

          <div class="selected-count">
            <strong>{{ wizard.normalizedTcpClients.value.length }}</strong>
            <span>TCP interfaces selected</span>
          </div>

          <div class="server-list">
            <label
              v-for="server in tcpServerOptions"
              :key="server.endpoint"
              class="server-option"
            >
              <input
                type="checkbox"
                :checked="wizard.selectedTcpEndpointSet.value.has(server.endpoint)"
                @change="wizard.setTcpEndpoint(server.endpoint, ($event.target as HTMLInputElement).checked)"
              />
              <span class="server-copy">
                <strong>{{ server.name }}</strong>
                <span>{{ server.endpoint }}</span>
              </span>
              <span v-if="server.isBootstrap" class="bootstrap-badge">Bootstrap</span>
            </label>
          </div>

          <div class="custom-row">
            <input
              v-model="wizard.customTcpEndpoint.value"
              type="text"
              placeholder="host:port"
              @keyup.enter="wizard.addCustomTcpEndpoint"
            />
            <button type="button" class="icon-action" aria-label="Add TCP endpoint" @click="wizard.addCustomTcpEndpoint">
              +
            </button>
          </div>

          <div class="active-endpoints">
            <span
              v-for="endpoint in wizard.normalizedTcpClients.value"
              :key="endpoint"
              class="active-endpoint"
            >
              {{ endpoint }}
              <button type="button" class="inline-remove" @click="wizard.removeTcpEndpoint(endpoint)">
                Remove
              </button>
            </span>
          </div>
        </section>

        <section v-else-if="wizard.activeStep.value.id === 'telemetry'" class="wizard-section">
          <div class="section-heading">
            <p class="eyebrow">Position sharing</p>
            <h1>Telemetry</h1>
            <p>Choose whether this node publishes position under the current call sign.</p>
          </div>
          <label class="toggle-card">
            <input v-model="wizard.draft.telemetryEnabled" type="checkbox" />
            <span class="toggle-visual" aria-hidden="true"></span>
            <span>
              <strong>Activate telemetry sharing</strong>
              <small>Location permission is requested during setup.</small>
            </span>
          </label>
          <dl class="config-grid">
            <div>
              <dt>Publish interval</dt>
              <dd>60s</dd>
            </div>
            <div>
              <dt>Stale after</dt>
              <dd>30 min</dd>
            </div>
            <div>
              <dt>Expires after</dt>
              <dd>180 min</dd>
            </div>
            <div>
              <dt>Call Sign</dt>
              <dd>{{ wizard.normalizedDisplayName.value || "Unset" }}</dd>
            </div>
          </dl>
          <div class="radar-panel">
            <span></span>
            <p>Permission required next</p>
          </div>
        </section>

        <section v-else-if="wizard.activeStep.value.id === 'permissions'" class="wizard-section">
          <div class="section-heading">
            <p class="eyebrow">Android permissions</p>
            <h1>Permissions</h1>
            <p>Grant access used by the features selected in setup.</p>
          </div>
          <div class="permission-grid">
            <div class="permission-card" :class="wizard.permissions.location">
              <div>
                <strong>Location</strong>
                <span>Required for telemetry</span>
                <small>{{ wizard.permissionLabel(wizard.permissions.location) }}</small>
              </div>
              <button
                type="button"
                :disabled="wizard.permissions.location === 'granted' || wizard.permissions.location === 'unavailable'"
                @click="wizard.requestLocation"
              >
                Request
              </button>
            </div>
            <div class="permission-card" :class="wizard.permissions.notifications">
              <div>
                <strong>Notifications</strong>
                <span>Recommended for alerts</span>
                <small>{{ wizard.permissionLabel(wizard.permissions.notifications) }}</small>
              </div>
              <button
                type="button"
                :disabled="wizard.permissions.notifications === 'granted' || wizard.permissions.notifications === 'unavailable'"
                @click="wizard.requestNotifications"
              >
                Request
              </button>
            </div>
          </div>
          <div class="status-strip">
            <span>Denied permissions do not block setup.</span>
            <strong>Review</strong>
          </div>
        </section>

        <section v-else-if="wizard.activeStep.value.id === 'sos'" class="wizard-section">
          <div class="section-heading">
            <p class="eyebrow">Emergency access</p>
            <h1>SOS</h1>
            <p>Enable rapid emergency activation for this node.</p>
          </div>
          <label class="toggle-card danger-card">
            <input v-model="wizard.draft.sosEnabled" type="checkbox" />
            <span class="toggle-visual" aria-hidden="true"></span>
            <span>
              <strong>Enable SOS</strong>
              <small>Also activates the floating SOS button.</small>
            </span>
          </label>
          <div class="sos-preview">
            <div>
              <span>Floating SOS button</span>
              <strong>{{ wizard.sosFloatingButtonEnabled.value ? "Enabled" : "Disabled" }}</strong>
            </div>
            <span class="sos-fab-preview">SOS</span>
          </div>
          <dl class="config-grid">
            <div>
              <dt>Countdown</dt>
              <dd>5s</dd>
            </div>
            <div>
              <dt>Include location</dt>
              <dd>Yes</dd>
            </div>
            <div>
              <dt>Periodic updates</dt>
              <dd>Off</dd>
            </div>
            <div>
              <dt>Audio recording</dt>
              <dd>Off</dd>
            </div>
          </dl>
        </section>

        <section v-else class="wizard-section">
          <div class="section-heading">
            <p class="eyebrow">Confirm configuration</p>
            <h1>Review Setup</h1>
            <p>Finish saves the first-run setup and opens the Dashboard.</p>
          </div>
          <dl class="review-grid">
            <div>
              <dt>Call Sign</dt>
              <dd>{{ wizard.normalizedDisplayName.value || "Required" }}</dd>
            </div>
            <div>
              <dt>TCP Interfaces</dt>
              <dd>{{ wizard.normalizedTcpClients.value.length }} selected</dd>
            </div>
            <div>
              <dt>Telemetry</dt>
              <dd>{{ wizard.draft.telemetryEnabled ? "Enabled" : "Disabled" }}</dd>
            </div>
            <div>
              <dt>Location</dt>
              <dd>{{ wizard.permissionLabel(wizard.permissions.location) }}</dd>
            </div>
            <div>
              <dt>Notifications</dt>
              <dd>{{ wizard.permissionLabel(wizard.permissions.notifications) }}</dd>
            </div>
            <div>
              <dt>SOS</dt>
              <dd>{{ wizard.draft.sosEnabled ? "Enabled" : "Disabled" }}</dd>
            </div>
            <div>
              <dt>Floating SOS</dt>
              <dd>{{ wizard.sosFloatingButtonEnabled.value ? "Enabled" : "Disabled" }}</dd>
            </div>
            <div>
              <dt>Bio Sensors</dt>
              <dd>Inactive</dd>
            </div>
          </dl>
          <div class="status-strip ready-strip">
            <span>Ready to save setup</span>
            <strong>Finish</strong>
          </div>
        </section>
      </article>

      <p v-if="wizard.feedback.value" class="feedback">{{ wizard.feedback.value }}</p>

      <footer class="wizard-actions">
        <button
          type="button"
          class="secondary-action"
          :disabled="wizard.activeIndex.value === 0 || wizard.saving.value"
          @click="wizard.back"
        >
          Back
        </button>
        <button
          v-if="wizard.activeStep.value.id !== 'review'"
          type="button"
          class="primary-action"
          :disabled="!wizard.canGoNext.value"
          @click="wizard.next"
        >
          {{ wizard.activeStep.value.id === "welcome" ? "Start Setup" : "Next" }}
        </button>
        <button
          v-else
          type="button"
          class="primary-action"
          :disabled="wizard.saving.value || !wizard.canGoNext.value"
          data-testid="setup-finish"
          @click="wizard.finish"
        >
          {{ wizard.saving.value ? "Saving" : "Finish" }}
        </button>
      </footer>
    </div>
  </section>
</template>

<style scoped>
.setup-view {
  --setup-border: rgb(74 133 207 / 45%);
  --setup-panel: linear-gradient(180deg, rgb(9 25 55 / 92%), rgb(7 16 37 / 96%));
  color: #def1ff;
  height: 100%;
  min-height: 0;
}

.setup-console {
  background:
    radial-gradient(circle at 50% 28%, rgb(100 190 255 / 13%), transparent 30%),
    linear-gradient(180deg, rgb(3 13 30 / 96%), #020710 82%);
  border: 1px solid rgb(64 190 255 / 34%);
  border-radius: 16px;
  box-sizing: border-box;
  box-shadow:
    inset 0 0 0 1px rgb(9 42 76 / 72%),
    0 18px 52px rgb(0 0 0 / 38%);
  display: grid;
  gap: 0.78rem;
  grid-template-rows: auto auto auto minmax(0, 1fr) auto;
  height: 100%;
  margin: 0 auto;
  max-width: 880px;
  min-height: 0;
  overflow: hidden;
  padding: clamp(0.76rem, 2vw, 1.2rem);
}

.console-header,
.step-band,
.wizard-actions,
.mission-row,
.server-option,
.custom-row,
.active-endpoint,
.permission-card,
.sos-preview,
.status-strip {
  align-items: center;
  display: flex;
}

.console-header {
  justify-content: space-between;
}

.brand-lockup {
  align-items: center;
  display: flex;
  gap: 0.78rem;
  min-width: 0;
}

.brand-symbol {
  filter: drop-shadow(0 0 14px rgb(100 190 255 / 28%));
  height: clamp(3.1rem, 10vw, 4.2rem);
  width: clamp(3.1rem, 10vw, 4.2rem);
}

.brand-copy {
  display: grid;
  gap: 0;
  min-width: 0;
  text-transform: uppercase;
}

.brand-copy p,
.brand-copy span,
.band-label,
.band-step,
.eyebrow,
.row-icon,
.bootstrap-badge,
.selected-count strong,
.selected-count span,
.config-grid dt,
.review-grid dt,
button {
  font-family: var(--font-ui);
}

.brand-copy p {
  color: #def1ff;
  font-size: clamp(1.65rem, 5vw, 2.7rem);
  font-weight: 800;
  letter-spacing: 0.15em;
  line-height: 0.86;
  margin: 0;
}

.brand-copy span {
  color: #9ee2ff;
  font-size: clamp(0.88rem, 2.8vw, 1.28rem);
  font-weight: 700;
  letter-spacing: 0.1em;
  line-height: 1;
}

.node-state {
  align-items: center;
  background: rgb(5 20 44 / 72%);
  border: 1px solid var(--setup-border);
  border-radius: 14px;
  color: #9cb3d6;
  display: flex;
  font-family: var(--font-ui);
  font-size: 0.74rem;
  gap: 0.5rem;
  letter-spacing: 0.12em;
  line-height: 1.1;
  padding: 0.56rem 0.72rem;
  text-transform: uppercase;
}

.node-state strong {
  color: #70f0a4;
}

.state-dot {
  background: #70f0a4;
  border-radius: 999px;
  box-shadow: 0 0 12px rgb(112 240 164 / 52%);
  height: 0.72rem;
  width: 0.72rem;
}

.step-band {
  background: var(--setup-panel);
  border: 1px solid var(--setup-border);
  border-radius: 14px;
  justify-content: space-between;
  min-height: 3.4rem;
  padding: 0.68rem 0.88rem;
}

.band-label,
.band-step,
.eyebrow {
  color: #9cb3d6;
  font-size: 0.78rem;
  font-weight: 800;
  letter-spacing: 0.12em;
  margin: 0;
  text-transform: uppercase;
}

.band-underline {
  background: #64beff;
  box-shadow: 0 0 16px rgb(100 190 255 / 44%);
  display: block;
  height: 2px;
  margin-top: 0.5rem;
  width: 6.4rem;
}

.wizard-panel {
  background:
    radial-gradient(circle at 50% 18%, rgb(100 190 255 / 8%), transparent 34%),
    var(--setup-panel);
  border: 1px solid rgb(74 133 207 / 38%);
  border-radius: 14px;
  min-height: 0;
  overflow-y: auto;
  scrollbar-gutter: stable;
}

.wizard-section {
  display: grid;
  gap: 0.9rem;
  padding: clamp(0.92rem, 2.4vw, 1.35rem);
}

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
  margin-top: 0.2rem;
}

.wizard-section h1 {
  color: #def1ff;
  font-family: var(--font-headline);
  font-size: clamp(2rem, 8vw, 4rem);
  font-weight: 800;
  letter-spacing: 0.04em;
  line-height: 0.95;
  margin: 0;
  text-transform: uppercase;
}

.section-heading {
  display: grid;
  gap: 0.34rem;
}

.section-heading h1 {
  font-size: clamp(1.9rem, 5.6vw, 3.2rem);
}

.intro-copy,
.section-heading p,
.mission-row small,
.server-copy span,
.active-endpoint,
.toggle-card small,
.permission-card span,
.permission-card small,
.config-grid dd,
.review-grid dd,
.preview-panel,
.feedback,
.radar-panel p,
.sos-preview span {
  color: #9cb3d6;
  font-family: var(--font-body);
}

.intro-copy,
.section-heading p,
.feedback {
  margin: 0;
}

.mission-list {
  border: 1px solid rgb(74 133 207 / 42%);
  border-radius: 14px;
  overflow: hidden;
  width: 100%;
}

.mission-row {
  background: rgb(5 20 44 / 58%);
  border-bottom: 1px solid rgb(74 133 207 / 32%);
  gap: 0.7rem;
  padding: 0.72rem 0.78rem;
  text-align: left;
}

.mission-row:last-child {
  border-bottom: 0;
}

.row-icon {
  align-items: center;
  border: 1px solid rgb(100 190 255 / 62%);
  border-radius: 10px;
  color: #64beff;
  display: inline-flex;
  flex: 0 0 2.65rem;
  font-size: 0.68rem;
  font-weight: 900;
  height: 2.65rem;
  justify-content: center;
}

.danger-row .row-icon {
  border-color: rgb(255 102 102 / 80%);
  color: #ff7777;
}

.mission-row div {
  display: grid;
  gap: 0.1rem;
}

.mission-row strong,
.server-copy strong,
.toggle-card strong,
.permission-card strong,
.preview-panel strong,
.status-strip strong,
.sos-preview strong {
  color: #def1ff;
  font-family: var(--font-ui);
}

.field-block {
  color: #9cb3d6;
  display: grid;
  font-family: var(--font-ui);
  font-size: 0.78rem;
  font-weight: 800;
  gap: 0.34rem;
  letter-spacing: 0.1em;
  text-transform: uppercase;
}

.field-block input,
.custom-row input {
  background: rgb(2 7 16 / 84%);
  border: 1px solid rgb(74 133 207 / 58%);
  border-radius: 14px;
  color: #def1ff;
  font-family: var(--font-body);
  font-size: 1.05rem;
  min-height: 3rem;
  padding: 0.58rem 0.72rem;
}

.field-block input:focus,
.custom-row input:focus {
  border-color: #64beff;
  box-shadow: 0 0 0 2px rgb(100 190 255 / 18%);
  outline: 0;
}

.status-strip {
  background: rgb(5 20 44 / 72%);
  border: 1px solid rgb(74 133 207 / 42%);
  border-radius: 14px;
  justify-content: space-between;
  padding: 0.68rem 0.76rem;
}

.status-strip.blocked {
  border-color: rgb(255 180 171 / 58%);
}

.status-strip.blocked strong {
  color: #ffb4ab;
}

.preview-panel,
.selected-count,
.radar-panel {
  background:
    linear-gradient(180deg, rgb(6 24 54 / 78%), rgb(4 15 34 / 88%));
  border: 1px solid rgb(74 133 207 / 36%);
  border-radius: 14px;
  padding: 0.82rem;
}

.preview-panel {
  display: grid;
  gap: 0.18rem;
}

.selected-count {
  align-items: baseline;
  display: flex;
  gap: 0.45rem;
}

.selected-count strong {
  color: #64beff;
  font-size: 1.8rem;
  line-height: 1;
}

.server-list,
.active-endpoints {
  display: grid;
  gap: 0.46rem;
}

.server-list {
  max-height: min(31dvh, 18rem);
  overflow-y: auto;
  padding-right: 0.18rem;
  scrollbar-gutter: stable;
}

.server-option {
  background: rgb(5 20 44 / 70%);
  border: 1px solid rgb(74 133 207 / 34%);
  border-radius: 14px;
  gap: 0.58rem;
  grid-template-columns: auto minmax(0, 1fr) auto;
  padding: 0.58rem 0.66rem;
}

.server-option input {
  accent-color: #64beff;
}

.server-copy {
  display: grid;
  gap: 0.08rem;
  min-width: 0;
}

.server-copy span,
.active-endpoint {
  overflow-wrap: anywhere;
}

.bootstrap-badge {
  border: 1px solid rgb(100 190 255 / 46%);
  border-radius: 999px;
  color: #64beff;
  font-size: 0.62rem;
  font-weight: 800;
  padding: 0.18rem 0.44rem;
  text-transform: uppercase;
}

.custom-row {
  gap: 0.5rem;
}

.custom-row input {
  flex: 1;
  min-width: 0;
}

.active-endpoint,
.sos-preview {
  background: rgb(2 7 16 / 58%);
  border: 1px solid rgb(74 133 207 / 28%);
  border-radius: 12px;
  justify-content: space-between;
  padding: 0.48rem 0.58rem;
}

.toggle-card,
.permission-card,
.config-grid div,
.review-grid div {
  background: rgb(5 20 44 / 72%);
  border: 1px solid rgb(74 133 207 / 38%);
  border-radius: 14px;
}

.toggle-card {
  align-items: center;
  display: grid;
  gap: 0.64rem;
  grid-template-columns: auto minmax(0, 1fr);
  padding: 0.78rem;
  position: relative;
}

.toggle-card > span:last-child {
  display: grid;
  gap: 0.1rem;
}

.toggle-card input {
  cursor: pointer;
  height: 1.8rem;
  left: 0.78rem;
  opacity: 0;
  position: absolute;
  top: 50%;
  transform: translateY(-50%);
  width: 3.2rem;
  z-index: 2;
}

.toggle-visual {
  background: rgb(2 7 16 / 76%);
  border: 1px solid rgb(74 133 207 / 52%);
  border-radius: 999px;
  height: 1.8rem;
  position: relative;
  width: 3.2rem;
}

.toggle-visual::after {
  background: #9cb3d6;
  border-radius: 999px;
  content: "";
  height: 1.14rem;
  left: 0.3rem;
  position: absolute;
  top: 0.28rem;
  width: 1.14rem;
}

.toggle-card input:checked + .toggle-visual {
  background: rgb(31 118 225 / 74%);
  border-color: #64beff;
}

.toggle-card input:checked + .toggle-visual::after {
  background: #64beff;
  box-shadow: 0 0 12px rgb(100 190 255 / 60%);
  transform: translateX(1.38rem);
}

.toggle-card input:focus-visible + .toggle-visual {
  box-shadow: 0 0 0 3px rgb(100 190 255 / 24%);
}

.danger-card {
  border-color: rgb(255 102 102 / 54%);
}

.danger-card input:checked + .toggle-visual {
  background: rgb(99 21 25 / 78%);
  border-color: #ff7777;
}

.danger-card input:checked + .toggle-visual::after {
  background: #ff7777;
  box-shadow: 0 0 12px rgb(255 102 102 / 62%);
}

.config-grid,
.review-grid {
  display: grid;
  gap: 0.56rem;
  grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
  margin: 0;
}

.permission-grid {
  display: grid;
  gap: 0.56rem;
  grid-template-columns: minmax(0, 1fr);
}

.config-grid div,
.review-grid div {
  display: grid;
  gap: 0.12rem;
  padding: 0.64rem;
}

.config-grid dt,
.review-grid dt {
  color: #64beff;
  font-size: 0.68rem;
  font-weight: 900;
  letter-spacing: 0.1em;
  text-transform: uppercase;
}

.config-grid dd,
.review-grid dd {
  margin: 0;
}

.radar-panel {
  align-items: center;
  display: flex;
  gap: 0.7rem;
}

.radar-panel span {
  background:
    radial-gradient(circle, rgb(100 190 255 / 52%) 0 12%, transparent 14% 100%),
    repeating-radial-gradient(circle, rgb(100 190 255 / 25%) 0 1px, transparent 2px 14px);
  border: 1px solid rgb(100 190 255 / 42%);
  border-radius: 999px;
  height: 3.4rem;
  width: 3.4rem;
}

.radar-panel p {
  margin: 0;
}

.permission-card {
  display: flex;
  gap: 0.7rem;
  justify-content: space-between;
  padding: 0.76rem;
}

.permission-card div {
  display: grid;
  gap: 0.1rem;
}

.permission-card.granted {
  border-color: rgb(112 240 164 / 58%);
}

.permission-card.granted small {
  color: #70f0a4;
}

.permission-card.denied {
  border-color: rgb(255 180 171 / 64%);
}

.permission-card.denied small {
  color: #ffb4ab;
}

.sos-preview {
  min-height: 4.8rem;
}

.sos-preview div {
  display: grid;
  gap: 0.2rem;
}

.sos-fab-preview {
  align-items: center;
  background: radial-gradient(circle at 30% 25%, #ffb4ab, #9f202b 62%, #4b0d16);
  border: 1px solid rgb(255 180 171 / 80%);
  border-radius: 999px;
  box-shadow: 0 0 24px rgb(255 89 89 / 42%);
  color: #fff7f5;
  display: inline-flex;
  flex: 0 0 3.8rem;
  font-family: var(--font-ui);
  font-weight: 900;
  height: 3.8rem;
  justify-content: center;
}

.ready-strip {
  border-color: rgb(112 240 164 / 46%);
}

.ready-strip strong {
  color: #70f0a4;
}

.feedback {
  margin: 0;
}

.wizard-actions {
  gap: 0.7rem;
  justify-content: space-between;
}

button {
  --btn-bg: linear-gradient(180deg, rgb(10 35 72 / 88%), rgb(6 24 54 / 92%));
  --btn-bg-pressed: linear-gradient(180deg, rgb(196 240 255 / 96%), rgb(118 212 255 / 94%));
  --btn-border: rgb(74 133 207 / 45%);
  --btn-border-pressed: rgb(224 248 255 / 86%);
  --btn-shadow: inset 0 1px 0 rgb(209 244 255 / 10%), 0 8px 18px rgb(2 14 32 / 18%);
  --btn-shadow-pressed: inset 0 1px 0 rgb(255 255 255 / 75%), 0 4px 10px rgb(3 21 47 / 24%);
  --btn-color: #8fdbff;
  --btn-color-pressed: #042541;
  background: var(--btn-bg);
  border: 1px solid var(--btn-border);
  border-radius: 999px;
  box-shadow: var(--btn-shadow);
  color: var(--btn-color);
  cursor: pointer;
  font-size: 0.78rem;
  font-weight: 900;
  letter-spacing: 0.08em;
  min-height: 2.6rem;
  padding: 0 0.92rem;
  text-transform: uppercase;
}

.primary-action {
  --btn-bg: linear-gradient(180deg, #64beff, #179ce8);
  --btn-border: rgb(214 248 255 / 76%);
  --btn-color: #03192f;
  flex: 1;
  font-size: clamp(0.88rem, 2.8vw, 1.24rem);
  min-height: 3.2rem;
}

.secondary-action {
  min-width: 7rem;
}

.icon-action {
  border-radius: 14px;
  flex: 0 0 3rem;
  font-size: 1.2rem;
  padding: 0;
}

.inline-remove {
  font-size: 0.64rem;
  min-height: 1.8rem;
  padding-inline: 0.55rem;
}

button:disabled {
  cursor: not-allowed;
  opacity: 0.48;
}

@media (max-width: 760px) {
  .setup-console {
    border-radius: 14px;
    gap: 0.54rem;
    padding: 0.72rem;
  }

  .brand-symbol {
    height: 3rem;
    width: 3rem;
  }

  .brand-copy p {
    font-size: 1.5rem;
  }

  .brand-copy span {
    font-size: 0.78rem;
  }

  .node-state {
    display: none;
  }

  .step-band {
    min-height: 2.48rem;
    padding-block: 0.42rem;
  }

  .wizard-section {
    gap: 0.58rem;
    padding: 0.72rem;
  }

  .welcome-symbol {
    width: clamp(4.1rem, 22vw, 5.2rem);
  }

  .wizard-section h1 {
    font-size: clamp(1.48rem, 7.4vw, 2.14rem);
  }

  .section-heading h1 {
    font-size: clamp(1.7rem, 8.2vw, 2.5rem);
  }

  .intro-copy,
  .section-heading p,
  .mission-row small,
  .toggle-card small,
  .permission-card span,
  .permission-card small,
  .config-grid dd,
  .review-grid dd,
  .preview-panel,
  .feedback,
  .radar-panel p,
  .sos-preview span {
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

  .wizard-panel {
    min-height: 0;
  }

  .field-block input,
  .custom-row input {
    min-height: 2.72rem;
  }

  .config-grid,
  .review-grid {
    gap: 0.46rem;
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .permission-grid {
    gap: 0.46rem;
    grid-template-columns: minmax(0, 1fr);
  }

  .config-grid div,
  .review-grid div {
    padding: 0.5rem;
  }

  .server-option {
    grid-template-columns: auto minmax(0, 1fr);
    padding: 0.48rem 0.54rem;
  }

  .bootstrap-badge {
    grid-column: 2;
    justify-self: start;
  }

  .toggle-card {
    grid-template-columns: auto minmax(0, 1fr);
    padding: 0.58rem;
  }

  .toggle-card input {
    left: 0.58rem;
  }

  .permission-card {
    align-items: stretch;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0.58rem;
  }

  .wizard-actions {
    gap: 0.56rem;
  }

  .primary-action {
    min-height: 2.85rem;
  }

  .secondary-action {
    min-width: 6.4rem;
  }
}
</style>
