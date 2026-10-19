<script setup lang="ts">
import { appVersion } from "../../utils/appVersion";

const aboutItems = [
  { label: "Application name", value: "R.E.M. (Reticulum Emergency Manager)" },
  {
    label: "Description",
    value: "Emergency coordination, messages, events, and telemetry over Reticulum mesh networks.",
  },
  { label: "Version", value: appVersion },
  { label: "License", value: "Eclipse Public License (EPL)" },
] as const;

const repositoryUrl = "https://github.com/FreeTAKTeam/reticulum_mobile_emergency_management";
</script>

<template>
  <section class="panel settings-card">
    <dl class="about-list">
      <div v-for="item in aboutItems" :key="item.label" class="about-row">
        <dt>{{ item.label }}</dt>
        <dd>{{ item.value }}</dd>
      </div>
      <div class="about-row">
        <dt>GitHub repository URL</dt>
        <dd>
          <a :href="repositoryUrl" target="_blank" rel="noreferrer">{{ repositoryUrl }}</a>
        </dd>
      </div>
    </dl>
  </section>
</template>
