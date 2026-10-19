<script setup lang="ts">
import NetworkFields from "../../components/settings/forms/NetworkFields.vue";
import type { NetworkDraft } from "../../components/settings/settingsFormSlices";

defineProps<{ draft: NetworkDraft }>();
const emit = defineEmits<{ feedback: [message: string] }>();
</script>

<template>
  <NetworkFields
    :draft="draft"
    :pair-to="{ name: 'settings-rnode-pairing' }"
    @feedback="emit('feedback', $event)"
  />
</template>
