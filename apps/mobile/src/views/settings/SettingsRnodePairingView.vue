<script setup lang="ts">
import { useRouter } from "vue-router";

import RnodePairingFields from "../../components/settings/forms/RnodePairingFields.vue";
import type { NetworkDraft } from "../../components/settings/settingsFormSlices";

defineProps<{ draft: NetworkDraft }>();
const router = useRouter();

async function returnToNetwork(): Promise<void> {
  await router.push({ name: "settings-network" });
}
</script>

<template>
  <RnodePairingFields :draft="draft" @done="returnToNetwork" />
</template>
