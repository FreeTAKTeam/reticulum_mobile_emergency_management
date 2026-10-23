<script setup lang="ts">
import type { TelemetryPublicationSnapshot } from "@reticulum/node-client";
import { telemetryDeliveryLabel } from "../../utils/telemetryDeliveryLabels";

defineProps<{ publication: TelemetryPublicationSnapshot | null }>();
</script>

<template>
  <section class="telemetry-delivery" aria-label="Remote telemetry delivery" aria-live="polite">
    <p v-if="!publication">Remote delivery status unavailable.</p>
    <template v-else>
      <p v-if="publication.localFixUpdatedAt !== null">
        Latest local fix: {{ new Date(publication.localFixUpdatedAt).toLocaleTimeString() }}
      </p>
      <p v-if="publication.localFixUpdatedAt === null">No local fix yet.</p>
      <p v-else-if="publication.recipients.length === 0">Stored locally · No selected recipients.</p>
      <ul v-else>
        <li v-for="recipient in publication.recipients" :key="recipient.destinationHex">
          <code :title="recipient.destinationHex">{{ recipient.destinationHex.slice(0, 8) }}</code> · {{ telemetryDeliveryLabel(recipient.state) }}
          <span> · Fix {{ new Date(recipient.fixUpdatedAt).toLocaleTimeString() }}</span>
          <span v-if="recipient.fixUpdatedAt !== publication.localFixUpdatedAt"> · Earlier fix</span>
          <span v-if="recipient.messageIdHex" :title="recipient.messageIdHex"> · {{ recipient.messageIdHex.slice(0, 8) }}</span>
        </li>
      </ul>
      <p v-if="publication.lastReceiveFailure" class="feedback" role="status">
        Last receive failure: {{ publication.lastReceiveFailure.reason }}
        · {{ publication.lastReceiveFailure.sourceHex?.slice(0, 8) || "Unknown source" }}
        <span v-if="publication.lastReceiveFailure.commandId"> · {{ publication.lastReceiveFailure.commandId }}</span>
      </p>
    </template>
  </section>
</template>

<style scoped>
.telemetry-delivery {
  color: #aacbe6;
  font-family: var(--font-body);
  font-size: 0.8rem;
  line-height: 1.5;
  overflow-wrap: anywhere;
}
.telemetry-delivery p { margin: 0.65rem 0; }
.telemetry-delivery ul { padding-left: 1.1rem; }
.telemetry-delivery code { font-family: var(--font-ui); }
</style>
