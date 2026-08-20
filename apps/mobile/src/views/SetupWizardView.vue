<script setup lang="ts">
import { onMounted } from "vue";
import WizardProgress from "../components/setup/WizardProgress.vue";
import SetupWelcomeStep from "../components/setup/SetupWelcomeStep.vue";
import { useSetupWizard } from "../composables/useSetupWizard";
import logoUrl from "../assets/rem-logo.png";
import { RNODE_FREQUENCY_MAX_HZ, RNODE_FREQUENCY_MIN_HZ } from "../utils/rnodeProfiles";
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
        <SetupWelcomeStep
          v-if="wizard.activeStep.value.id === 'welcome'"
          :logo-url="logoUrl"
        />

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

        <section v-else-if="wizard.activeStep.value.id === 'rnode'" class="wizard-section">
          <div class="section-heading">
            <p class="eyebrow">RNode Bluetooth LoRa</p>
            <h1>LoRa Interface</h1>
            <p>Pair an RNode over BLE or Bluetooth Classic and choose the shared REM radio profile.</p>
          </div>
          <label class="toggle-card">
            <input v-model="wizard.draft.rnode.enabled" type="checkbox" />
            <span class="toggle-visual" aria-hidden="true"></span>
            <span>
              <strong>Enable RNode LoRa</strong>
              <small>Runs alongside configured TCP interfaces.</small>
            </span>
          </label>
          <label class="field-block">
            <span>Bluetooth bearer</span>
            <select v-model="wizard.draft.rnode.connectionMode">
              <option value="ble">Bluetooth Low Energy (BLE)</option>
              <option value="bluetooth_classic">Bluetooth Classic (SPP)</option>
            </select>
          </label>
          <div class="custom-row">
            <input v-model="wizard.draft.rnode.peripheralId" type="text" placeholder="RNode Bluetooth device id" />
            <button type="button" class="icon-action"
              :disabled="wizard.rnodePairedLoading.value" aria-label="Show paired Bluetooth devices"
              @click="wizard.loadPairedRnodeDevices"
            >
              {{ wizard.rnodePairedLoading.value ? "..." : "BT" }}
            </button>
          </div>
          <button
            type="button"
            class="secondary-action inline-action"
            :disabled="wizard.rnodeScanning.value"
            @click="wizard.scanRnodeDevices"
          >
            {{ wizard.rnodeScanning.value ? "Scanning" : wizard.draft.rnode.connectionMode === "bluetooth_classic" ? "Scan RNode Classic" : "Scan RNode BLE" }}
          </button>
          <button
            type="button"
            class="secondary-action inline-action"
            :disabled="wizard.rnodeUsbPairing.value"
            @click="wizard.pairRnodeViaUsb"
          >
            {{ wizard.rnodeUsbPairing.value ? "Pairing via USB" : "Pair via USB" }}
          </button>
          <p v-if="wizard.feedback.value" class="feedback">{{ wizard.feedback.value }}</p>
          <div class="server-list" v-if="wizard.rnodeUsbDevices.value.length > 0">
            <button
              v-for="device in wizard.rnodeUsbDevices.value"
              :key="`usb-${device.deviceId}`"
              type="button"
              class="server-option device-option"
              @click="wizard.selectRnodeUsbDevice(device)"
            >
              <span class="server-copy">
                <strong>{{ device.productName || device.deviceName || `USB ${device.deviceId}` }}</strong>
                <span>{{ wizard.rnodeUsbDeviceDetail(device) }}</span>
              </span>
              <span class="bootstrap-badge">
                {{ wizard.selectedRnodeUsbDeviceId.value === device.deviceId ? "Selected" : "USB" }}
              </span>
            </button>
          </div>
          <div class="server-list" v-if="wizard.rnodePairedDevices.value.length > 0">
            <button
              v-for="device in wizard.rnodePairedDevices.value"
              :key="`paired-${device.id}`"
              type="button"
              class="server-option device-option"
              @click="wizard.selectRnodeDevice(device)"
            >
              <span class="server-copy">
                <strong>{{ device.name || device.address }}</strong>
                <span>{{ wizard.rnodeDeviceDetail(device) }}</span>
              </span>
              <span class="bootstrap-badge">Paired</span>
            </button>
          </div>
          <div class="server-list" v-if="wizard.rnodeDevices.value.length > 0">
            <button
              v-for="device in wizard.rnodeDevices.value"
              :key="device.id"
              type="button"
              class="server-option device-option"
              @click="wizard.selectRnodeDevice(device)"
            >
              <span class="server-copy">
                <strong>{{ device.name || device.address }}</strong>
                <span>{{ wizard.rnodeDeviceDetail(device) }}</span>
              </span>
              <span class="bootstrap-badge">RNode</span>
            </button>
          </div>
          <label class="field-block">
            <span>Region</span>
            <select :value="wizard.draft.rnode.region" @change="wizard.selectRnodeRegion">
              <option v-for="region in wizard.rnodeRegions" :key="region.id" :value="region.id">
                {{ region.id }} - {{ region.label }}
              </option>
            </select>
          </label>
          <label class="field-block">
            <span>Frequency (Hz)</span>
            <input
              v-model.number="wizard.draft.rnode.frequencyHz"
              type="number"
              :min="RNODE_FREQUENCY_MIN_HZ"
              :max="RNODE_FREQUENCY_MAX_HZ"
              step="1000"
            />
          </label>
          <button type="button" class="secondary-action inline-action" @click="wizard.inferRnodeRegion">
            Infer region
          </button>
          <label class="field-block">
            <span>REM LoRa profile</span>
            <select v-model="wizard.draft.rnode.profile">
              <option v-for="profile in wizard.rnodeProfiles" :key="profile.id" :value="profile.id">
                {{ profile.id }} - {{ profile.label }}
              </option>
            </select>
          </label>
          <div class="status-strip">
            <span>{{ wizard.profileSummary() }}</span>
            <strong>{{ wizard.draft.rnode.enabled ? "Enabled" : "Disabled" }}</strong>
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
          <label class="field-block">
            <span>Telemetry publish interval (seconds)</span>
            <input
              v-model.number="wizard.draft.telemetryPublishIntervalSeconds"
              type="number"
              min="1"
              step="1"
              inputmode="numeric"
            />
          </label>
          <dl class="config-grid">
            <div>
              <dt>Publish interval</dt>
              <dd>{{ wizard.normalizedTelemetryPublishIntervalSeconds.value }}s</dd>
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
            <div class="permission-card" :class="wizard.permissions.bluetooth">
              <div>
                <strong>Bluetooth</strong>
                <span>Required for RNode LoRa</span>
                <small>{{ wizard.permissionLabel(wizard.permissions.bluetooth) }}</small>
              </div>
              <button
                type="button"
                :disabled="wizard.permissions.bluetooth === 'granted' || wizard.permissions.bluetooth === 'unavailable'"
                @click="wizard.requestBluetooth"
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
              <dt>RNode LoRa</dt>
              <dd>{{ wizard.draft.rnode.enabled ? wizard.draft.rnode.profile : "Disabled" }}</dd>
            </div>
            <div>
              <dt>RNode Device</dt>
              <dd>{{ wizard.draft.rnode.peripheralId || "Not selected" }}</dd>
            </div>
            <div>
              <dt>Telemetry Interval</dt>
              <dd>{{ wizard.normalizedTelemetryPublishIntervalSeconds.value }}s</dd>
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
              <dt>Bluetooth</dt>
              <dd>{{ wizard.permissionLabel(wizard.permissions.bluetooth) }}</dd>
            </div>
            <div>
              <dt>SOS</dt>
              <dd>{{ wizard.draft.sosEnabled ? "Enabled" : "Disabled" }}</dd>
            </div>
            <div>
              <dt>Floating SOS</dt>
              <dd>{{ wizard.sosFloatingButtonEnabled.value ? "Enabled" : "Disabled" }}</dd>
            </div>
          </dl>
          <div class="status-strip ready-strip">
            <span>Ready to save setup</span>
            <strong>Finish</strong>
          </div>
        </section>
      </article>

      <p v-if="wizard.feedback.value && wizard.activeStep.value.id !== 'rnode'" class="feedback">{{ wizard.feedback.value }}</p>

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

<style scoped src="./SetupWizardLayout.css"></style>
<style scoped src="./SetupWizardControls.css"></style>
