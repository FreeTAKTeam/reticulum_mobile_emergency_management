<script setup lang="ts">
import { PhCaretRight as CaretRight } from "@phosphor-icons/vue";
import { computed, ref } from "vue";
import { RouterLink, type RouteLocationRaw } from "vue-router";

import {
  TCP_COMMUNITY_SERVERS,
  normalizeTcpEndpointInput,
  toTcpEndpoint,
} from "../../../utils/tcpCommunityServers";
import {
  RNODE_FREQUENCY_MAX_HZ,
  RNODE_FREQUENCY_MIN_HZ,
  RNODE_PROFILE_SPECS,
  RNODE_REGION_SPECS,
  normalizeRnodeRegion,
  resolveRnodeFrequencyForRegionChange,
  rnodeProfileSummary,
  rnodeRegionDefaultFrequencyHz,
} from "../../../utils/rnodeProfiles";
import { normalizedTcpClients, type NetworkDraft } from "../settingsFormSlices";

/**
 * Network form body, shared by Settings > Network and the setup wizard. With `pairTo`
 * the "Pair RNode device" row links to that route; without it the row emits `pair`.
 */
const props = defineProps<{ draft: NetworkDraft; pairTo?: RouteLocationRaw }>();
const emit = defineEmits<{ feedback: [message: string]; pair: [] }>();
const customTcpEndpoint = ref("");

const knownTcpServers = TCP_COMMUNITY_SERVERS.map((server) => ({
  name: server.name,
  endpoint: toTcpEndpoint(server),
  isBootstrap: Boolean(server.isBootstrap),
}));
const activeEndpoints = computed(() => normalizedTcpClients(props.draft.tcpClients));
const selectedEndpoints = computed(() => new Set(activeEndpoints.value));
const pairedDeviceLabel = computed(() =>
  props.draft.rnodeDisplayName.trim() || props.draft.rnodePeripheralId.trim() || "No device selected",
);

function toggleKnownEndpoint(endpoint: string, selected: boolean): void {
  const next = new Set(activeEndpoints.value);
  if (selected) next.add(endpoint);
  else next.delete(endpoint);
  props.draft.tcpClients = [...next];
}

function addCustomEndpoint(): void {
  const normalized = normalizeTcpEndpointInput(customTcpEndpoint.value);
  if (!normalized) {
    emit("feedback", "Invalid endpoint. Use host:port, tcp://host:port, or [ipv6]:port.");
    return;
  }
  props.draft.tcpClients = [...new Set([...activeEndpoints.value, normalized])];
  customTcpEndpoint.value = "";
  emit("feedback", "");
}

function removeEndpoint(endpoint: string): void {
  props.draft.tcpClients = activeEndpoints.value.filter((entry) => entry !== endpoint);
}

function handleRegionChange(event: Event): void {
  const target = event.target;
  if (!(target instanceof HTMLSelectElement)) return;
  const previousRegion = normalizeRnodeRegion(props.draft.rnodeRegion);
  const nextRegion = normalizeRnodeRegion(target.value);
  props.draft.rnodeRegion = nextRegion;
  props.draft.rnodeFrequencyHz = resolveRnodeFrequencyForRegionChange(
    previousRegion,
    nextRegion,
    props.draft.rnodeFrequencyHz,
  );
}
</script>

<template>
  <section class="panel settings-card" aria-labelledby="tcp-interfaces-heading">
    <div class="config-section-header">
      <h3 id="tcp-interfaces-heading">TCP <span class="restart-badge">restart</span></h3>
      <p>Community servers and your own host:port endpoints.</p>
    </div>
    <div class="server-list">
      <label v-for="server in knownTcpServers" :key="server.endpoint" class="server-option">
        <input
          type="checkbox"
          :checked="selectedEndpoints.has(server.endpoint)"
          @change="toggleKnownEndpoint(server.endpoint, ($event.target as HTMLInputElement).checked)"
        />
        <div class="server-option-body">
          <p class="server-name">{{ server.name }}</p>
          <p class="server-endpoint">{{ server.endpoint }}</p>
        </div>
        <span v-if="server.isBootstrap" class="bootstrap-badge">Bootstrap</span>
      </label>
    </div>
    <div class="tcp-custom-row">
      <input
        v-model="customTcpEndpoint"
        type="text"
        placeholder="Add custom endpoint (host:port or tcp://host:port)"
        @keydown.enter.prevent="addCustomEndpoint"
      />
      <button type="button" @click="addCustomEndpoint">Add</button>
    </div>
    <div v-if="activeEndpoints.length > 0" class="active-endpoints">
      <article v-for="endpoint in activeEndpoints" :key="endpoint" class="active-endpoint">
        <span>{{ endpoint }}</span>
        <button type="button" class="inline-remove" @click="removeEndpoint(endpoint)">Remove</button>
      </article>
    </div>
    <p v-else class="section-note">No TCP endpoints configured.</p>
  </section>

  <section class="panel settings-card" aria-labelledby="lora-interface-heading">
    <div class="config-section-header">
      <h3 id="lora-interface-heading">LoRa / RNode <span class="restart-badge">applies on save</span></h3>
      <p>A paired RNode radio and the REM radio profile.</p>
    </div>
    <label class="checkbox">
      <input v-model="draft.rnodeEnabled" type="checkbox" />
      Enable RNode Bluetooth LoRa
    </label>
    <template v-if="draft.rnodeEnabled">
      <component
        :is="pairTo ? RouterLink : 'button'"
        :to="pairTo"
        :type="pairTo ? undefined : 'button'"
        class="settings-row task-row"
        @click="pairTo ? undefined : emit('pair')"
      >
        <span class="settings-row-copy">
          <strong>Pair RNode device</strong>
          <small>{{ pairedDeviceLabel }}</small>
        </span>
        <CaretRight :size="18" aria-hidden="true" />
      </component>
      <div class="grid">
        <label>
          Region
          <select :value="draft.rnodeRegion" @change="handleRegionChange">
            <option v-for="region in RNODE_REGION_SPECS" :key="region.id" :value="region.id">
              {{ region.id }} - {{ region.label }}
            </option>
          </select>
        </label>
        <label>
          REM LoRa profile
          <select v-model="draft.rnodeProfile">
            <option v-for="profile in RNODE_PROFILE_SPECS" :key="profile.id" :value="profile.id">
              {{ profile.id }} - {{ profile.label }}
            </option>
          </select>
        </label>
        <label>
          Frequency (Hz)
          <input
            v-model.number="draft.rnodeFrequencyHz"
            type="number"
            :min="RNODE_FREQUENCY_MIN_HZ"
            :max="RNODE_FREQUENCY_MAX_HZ"
            step="1000"
            :placeholder="String(rnodeRegionDefaultFrequencyHz(normalizeRnodeRegion(draft.rnodeRegion)))"
          />
        </label>
      </div>
      <dl class="status-rows">
        <div class="status-row">
          <dt>Reticulum syntax</dt>
          <dd><code>{{ rnodeProfileSummary(draft.rnodeProfile) }}</code></dd>
        </div>
      </dl>
    </template>
    <p class="section-note">When TCP and LoRa are both active, Reticulum selects the route.</p>
  </section>

  <details class="panel settings-advanced">
    <summary>Advanced</summary>
    <div class="settings-fields">
      <label class="checkbox">
        <input v-model="draft.broadcast" type="checkbox" />
        Broadcast enabled <span class="restart-badge">restart</span>
      </label>
      <label class="checkbox">
        <input v-model="draft.transportNodeEnabled" type="checkbox" />
        Transport node forwarding <span class="restart-badge">restart</span>
      </label>
    </div>
  </details>
</template>
