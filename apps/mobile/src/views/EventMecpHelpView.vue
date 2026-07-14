<script setup lang="ts">
import { computed } from "vue";
import { RouterLink } from "vue-router";

import {
  MECP_CATEGORIES,
  MECP_SEVERITIES,
  type MecpCategoryCode,
} from "../utils/mecp";

type SeverityTone = "red" | "yellow" | "green" | "unknown";

interface SeverityHelpItem {
  label: string;
  meaning: string;
  status: string;
  tone: SeverityTone;
  value: number;
}

interface CategoryHelpItem {
  code: MecpCategoryCode;
  label: string;
  summary: string;
}

const severityToneByStatus: Record<string, SeverityTone> = {
  Green: "green",
  Red: "red",
  Unknown: "unknown",
  Yellow: "yellow",
};

const severityItems = computed<SeverityHelpItem[]>(() =>
  MECP_SEVERITIES.map((severity) => ({
    label: severity.label,
    meaning: severity.meaning,
    status: severity.status,
    tone: severityToneByStatus[severity.status] ?? "unknown",
    value: severity.value,
  })),
);

const categorySummaries: Record<MecpCategoryCode, string> = {
  B: "Distress beacons and beacon cancellation.",
  C: "Requests, relays, checks, and meeting points.",
  D: "Drills, tests, and messages sent by mistake.",
  H: "Resources available to share with others.",
  L: "Low-risk everyday notes for practice and check-ins.",
  M: "Injuries, casualties, medical needs, or searched areas.",
  P: "Movement, sheltering, routes, or people stuck somewhere.",
  R: "Acknowledgements, help status, ETA, or all clear.",
  S: "Water, food, medication, fuel, power, and tools.",
  T: "Roads, bridges, fire, flooding, buildings, and hazards.",
  W: "Storms, visibility, heat, cold, and air quality.",
  X: "Threats, unsafe areas, unrest, or checkpoints.",
};

const primaryCategoryCodes: MecpCategoryCode[] = ["M", "T", "W", "S", "P", "C", "R", "X"];
const categoryItems = computed<CategoryHelpItem[]>(() =>
  primaryCategoryCodes.map((code) => {
    const category = MECP_CATEGORIES.find((item) => item.code === code);
    return {
      code,
      label: category?.label ?? code,
      summary: categorySummaries[code],
    };
  }),
);

const exampleParts = [
  {
    key: "protocol",
    value: "MECP",
    label: "Protocol",
    detail: "This is an MECP event.",
  },
  {
    key: "severity",
    value: "2",
    label: "Safety",
    detail: "Safe update, not a distress call.",
  },
  {
    key: "category",
    value: "P",
    label: "Position",
    detail: "Position or movement category.",
  },
  {
    key: "event",
    value: "01",
    label: "Stranded",
    detail: "Stranded or stuck.",
  },
];

const workflowSteps = [
  "Choose severity",
  "Pick category",
  "Select event",
  "Add details",
  "Send short body",
];
</script>

<template>
  <section class="mecp-help-view">
    <section class="utility-row" aria-label="MECP help controls">
      <div class="utility-chip">
        <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <path d="M12 4 4 8l8 4 8-4-8-4Z" />
          <path d="M4 12l8 4 8-4" />
          <path d="M4 16l8 4 8-4" />
        </svg>
        <span>Event Codes</span>
      </div>
      <div class="utility-chip filter-chip">
        <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <path d="M4 5h16l-6 7v5l-4 2v-7L4 5Z" />
        </svg>
        <span>Filter: All</span>
        <svg class="chevron" viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <path d="m7 10 5 5 5-5" />
        </svg>
      </div>
      <RouterLink to="/events" class="utility-chip back-chip">
        <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <path d="M15 6 9 12l6 6" />
          <path d="M10 12h10" />
        </svg>
        <span>Events</span>
      </RouterLink>
    </section>

    <section class="intro-panel" aria-label="MECP summary">
      <div class="intro-icon" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="none">
          <path d="M12 3v4" />
          <path d="M12 17v4" />
          <path d="M4 12h4" />
          <path d="M16 12h4" />
          <circle cx="12" cy="12" r="4" />
        </svg>
      </div>
      <div class="intro-copy">
        <p class="eyebrow">Mesh Emergency Communication Protocol</p>
        <h2>MECP turns an event into a short body peers can read quickly.</h2>
        <p>
          Use it when bandwidth is limited, stress is high, or teams need clear common wording.
        </p>
      </div>
    </section>

    <section class="example-panel" aria-label="MECP example">
      <header class="panel-heading">
        <span class="code-tag">Example</span>
        <strong>MECP/2/P01</strong>
      </header>
      <div class="example-grid">
        <article v-for="part in exampleParts" :key="part.key" class="example-card">
          <span>{{ part.value }}</span>
          <div>
            <h3>{{ part.label }}</h3>
            <p>{{ part.detail }}</p>
          </div>
        </article>
      </div>
    </section>

    <section class="severity-panel" aria-label="MECP severity levels">
      <header class="section-heading">
        <h2>Severity</h2>
        <p>Start with how urgent the event is.</p>
      </header>
      <div class="severity-grid">
        <article
          v-for="item in severityItems"
          :key="item.value"
          class="severity-card"
          :class="`severity-card-${item.tone}`"
        >
          <span class="severity-dot" aria-hidden="true"></span>
          <strong>{{ item.label }}</strong>
          <p>{{ item.meaning }}</p>
          <small>Code {{ item.value }}</small>
        </article>
      </div>
    </section>

    <section class="category-panel" aria-label="MECP categories">
      <header class="section-heading">
        <h2>Categories</h2>
        <p>Then choose the kind of event.</p>
      </header>
      <div class="category-list">
        <article v-for="item in categoryItems" :key="item.code" class="category-row">
          <span class="category-code">{{ item.code }}</span>
          <div>
            <h3>{{ item.label }}</h3>
            <p>{{ item.summary }}</p>
          </div>
        </article>
      </div>
    </section>

    <section class="workflow-panel" aria-label="MECP workflow">
      <header class="workflow-title">
        <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <path d="M8 5h8a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2Z" />
          <path d="M9.5 4h5a1 1 0 0 1 1 1v1h-7V5a1 1 0 0 1 1-1Z" />
          <path d="m9.2 11 1 1 2-2" />
          <path d="M14 11h2" />
          <path d="m9.2 15 1 1 2-2" />
          <path d="M14 15h2" />
        </svg>
        <h2>How To Send One</h2>
      </header>
      <ol class="workflow-list">
        <li v-for="step in workflowSteps" :key="step">
          <span>{{ step }}</span>
        </li>
      </ol>
    </section>
  </section>
</template>

<style scoped src="./EventMecpHelpView.css"></style>
