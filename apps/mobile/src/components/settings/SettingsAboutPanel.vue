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
  <details class="panel fold-panel">
    <summary class="panel-summary">
      <div class="summary-copy">
        <span class="summary-icon" aria-hidden="true">
          <svg class="summary-icon-svg" viewBox="0 0 24 24" fill="none">
            <circle cx="12" cy="12" r="8" />
            <path d="M12 10.8v5.4" />
            <path d="M12 7.8h.01" />
          </svg>
        </span>
        <h2>About</h2>
        <p>Application information</p>
      </div>
      <span class="chevron" aria-hidden="true">&#9662;</span>
    </summary>
    <div class="panel-body">
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
    </div>
  </details>
</template>
