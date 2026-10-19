<script setup lang="ts">
import { PhArrowLeft as ArrowLeft } from "@phosphor-icons/vue";
import { useRouter, type RouteLocationRaw } from "vue-router";

// Shared, already prefixed with .settings-view: imported as plain CSS so no
// component's scoped compilation can claim them in the production bundle.
import "../../views/styles/settings-layout.css";
import "../../views/styles/settings-content.css";
import "../../views/styles/settings-pages.css";

const props = withDefaults(defineProps<{
  title: string;
  description?: string;
  back?: RouteLocationRaw;
  backLabel?: string;
}>(), {
  description: "",
  back: "/settings",
  backLabel: "Back to settings",
});

const router = useRouter();

async function goBack(): Promise<void> {
  await router.push(props.back);
}
</script>

<template>
  <section class="settings-view settings-page">
    <header class="settings-page-header">
      <button type="button" class="settings-back" @click="goBack">
        <ArrowLeft :size="20" aria-hidden="true" />
        <span>{{ backLabel }}</span>
      </button>
      <div class="settings-page-title">
        <h2>{{ title }}</h2>
        <p v-if="description">{{ description }}</p>
      </div>
    </header>
    <slot />
    <slot name="footer" />
  </section>
</template>
