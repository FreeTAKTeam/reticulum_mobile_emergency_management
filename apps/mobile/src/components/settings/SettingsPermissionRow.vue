<script setup lang="ts">
import { computed, onMounted, ref } from "vue";

import type { SetupPermissionState } from "../../services/setupPermissions";

const props = defineProps<{
  label: string;
  check: () => Promise<SetupPermissionState>;
  request: () => Promise<SetupPermissionState>;
}>();

const state = ref<SetupPermissionState | "checking">("checking");
const requesting = ref(false);

const STATE_LABELS: Record<SetupPermissionState | "checking", string> = {
  checking: "Checking…",
  granted: "Allowed",
  denied: "Denied — change it in Android settings",
  prompt: "Not asked yet",
  unavailable: "Not available on this device",
};
const stateLabel = computed(() => STATE_LABELS[state.value]);
const canRequest = computed(() => state.value === "prompt" || state.value === "denied");

async function refresh(): Promise<void> {
  try {
    state.value = await props.check();
  } catch {
    state.value = "unavailable";
  }
}

async function requestPermission(): Promise<void> {
  requesting.value = true;
  try {
    state.value = await props.request();
  } catch {
    state.value = "unavailable";
  } finally {
    requesting.value = false;
  }
}

onMounted(refresh);
</script>

<template>
  <div class="status-row permission-row">
    <dt>{{ label }}</dt>
    <dd>
      <span>{{ stateLabel }}</span>
      <button v-if="canRequest" type="button" :disabled="requesting" @click="requestPermission">
        {{ requesting ? "Asking" : "Request" }}
      </button>
    </dd>
  </div>
</template>
