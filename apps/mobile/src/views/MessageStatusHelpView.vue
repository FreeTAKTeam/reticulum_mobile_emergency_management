<script setup lang="ts">
import { RouterLink } from "vue-router";

type StatusTone = "red" | "yellow" | "green" | "unknown";
type LineIcon = "capability" | "medical" | "preparedness" | "security";

interface StatusDefinition {
  body: string[];
  heading: string;
  tone: StatusTone;
}

interface HelpLineSection {
  icon: LineIcon;
  label: string;
  line: string;
  statuses: StatusDefinition[];
}

const statusLegend: Array<{ tone: StatusTone; label: string; summary: string }> = [
  { tone: "red", label: "Red", summary: "Critical" },
  { tone: "yellow", label: "Yellow", summary: "Limited" },
  { tone: "green", label: "Green", summary: "Adequate" },
  { tone: "unknown", label: "Unknown", summary: "Not Confirmed" },
];

const rulePoints = [
  "Use the lowest accurate color when unsure.",
  "If conditions cannot be confirmed, select Unknown.",
  "Reassess and resend if conditions change materially.",
];

const operationalGuidance = [
  "Always select the lowest accurate color when uncertain.",
  "Reassess and resend the action message if conditions change materially.",
  "Unknown status must be resolved as soon as practical.",
  "Consistency across the team improves prioritization.",
  "Color inflation reduces credibility and response effectiveness.",
];

const lineSections: HelpLineSection[] = [
  {
    icon: "security",
    label: "Security",
    line: "3",
    statuses: [
      {
        tone: "red",
        heading: "Threats Imminent",
        body: ["Active hostile presence or credible threat."],
      },
      {
        tone: "yellow",
        heading: "Not Secure No Immediate Threat",
        body: ["Area unstable or perimeter degraded."],
      },
      {
        tone: "green",
        heading: "Secure",
        body: ["No active threats.", "Controlled access."],
      },
      {
        tone: "unknown",
        heading: "Not Confirmed",
        body: ["No reliable information.", "Cannot assess."],
      },
    ],
  },
  {
    icon: "capability",
    label: "Capability",
    line: "4",
    statuses: [
      {
        tone: "red",
        heading: "No Defensive Capability",
        body: ["No weapons, no trained defenders."],
      },
      {
        tone: "yellow",
        heading: "Limited Capability",
        body: ["Limited ammo or personnel.", "Short-term only."],
      },
      {
        tone: "green",
        heading: "Fully Capable",
        body: ["Weapons, ammo, and personnel ready."],
      },
      {
        tone: "unknown",
        heading: "Not Confirmed",
        body: ["Inventory or equipment status unclear."],
      },
    ],
  },
  {
    icon: "preparedness",
    label: "Preparedness",
    line: "5",
    statuses: [
      {
        tone: "red",
        heading: "No Sustainment Supplies",
        body: ["Food, water, fuel, or power < 24 hrs."],
      },
      {
        tone: "yellow",
        heading: "Limited Supplies",
        body: ["Supplies available < 1 week."],
      },
      {
        tone: "green",
        heading: "Adequate Supplies",
        body: ["Food, water, power, and essentials sufficient."],
      },
      {
        tone: "unknown",
        heading: "Not Confirmed",
        body: ["Inventory not checked.", "Consumption rate unknown."],
      },
    ],
  },
  {
    icon: "medical",
    label: "Medical",
    line: "6",
    statuses: [
      {
        tone: "red",
        heading: "Urgent Medical Need",
        body: ["Life-threatening injury or instability."],
      },
      {
        tone: "yellow",
        heading: "Delayed Care Acceptable",
        body: ["Minor injuries or stable conditions."],
      },
      {
        tone: "green",
        heading: "No Medical Issue",
        body: ["No injuries.", "All members stable."],
      },
      {
        tone: "unknown",
        heading: "Not Confirmed",
        body: ["Headcount or assessment incomplete."],
      },
    ],
  },
];

function statusLabel(tone: StatusTone): string {
  if (tone === "red") {
    return "RED";
  }
  if (tone === "yellow") {
    return "YELLOW";
  }
  if (tone === "green") {
    return "GREEN";
  }
  return "UNKNOWN";
}
</script>

<template>
  <section class="status-help-view">
    <section class="utility-row" aria-label="Status help controls">
      <div class="utility-chip">
        <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <path d="M12 4 4 8l8 4 8-4-8-4Z" />
          <path d="M4 12l8 4 8-4" />
          <path d="M4 16l8 4 8-4" />
        </svg>
        <span>Action Message Lines 3-8</span>
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
      <RouterLink to="/messages" class="utility-chip back-chip">
        <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <path d="M15 6 9 12l6 6" />
          <path d="M10 12h10" />
        </svg>
        <span>Messages</span>
      </RouterLink>
    </section>

    <section class="rule-panel" aria-label="Status color rule">
      <div class="rule-icon" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="none">
          <path d="M12 10v7" />
          <path d="M12 7h.01" />
          <circle cx="12" cy="12" r="8.5" />
        </svg>
      </div>
      <div class="rule-copy">
        <h2>Set the color for what is true now.</h2>
        <ul>
          <li v-for="point in rulePoints" :key="point">{{ point }}</li>
        </ul>
      </div>
      <ul class="status-legend" aria-label="Status legend">
        <li v-for="item in statusLegend" :key="item.tone" :class="`legend-${item.tone}`">
          <span class="legend-dot" aria-hidden="true"></span>
          <strong>{{ item.label }}</strong>
          <span>{{ item.summary }}</span>
        </li>
      </ul>
    </section>

    <section class="line-stack" aria-label="Status definitions by line">
      <article v-for="section in lineSections" :key="section.line" class="line-panel">
        <header class="line-title-row">
          <div class="line-icon" aria-hidden="true">
            <svg v-if="section.icon === 'security'" viewBox="0 0 24 24" fill="none">
              <path d="M12 3.5 19 6v5.4c0 4.2-2.8 7.8-7 9.1-4.2-1.3-7-4.9-7-9.1V6l7-2.5Z" />
              <path d="M9 12.2 11 14l4-5" />
            </svg>
            <svg v-else-if="section.icon === 'capability'" viewBox="0 0 24 24" fill="none">
              <path d="M12 3.5 19 6v5.4c0 4.2-2.8 7.8-7 9.1-4.2-1.3-7-4.9-7-9.1V6l7-2.5Z" />
              <path d="m12 8.4 1.1 2.3 2.5.35-1.8 1.75.42 2.5L12 14.1l-2.22 1.2.42-2.5-1.8-1.75 2.5-.35L12 8.4Z" />
            </svg>
            <svg v-else-if="section.icon === 'preparedness'" viewBox="0 0 24 24" fill="none">
              <path d="m4.5 8 7.5-4 7.5 4-7.5 4-7.5-4Z" />
              <path d="M4.5 8v8l7.5 4 7.5-4V8" />
              <path d="M12 12v8" />
              <path d="M8 10.2v3.2l2 1.1v-3.2L8 10.2Z" />
            </svg>
            <svg v-else viewBox="0 0 24 24" fill="none">
              <path d="M10 4h4v6h6v4h-6v6h-4v-6H4v-4h6V4Z" />
            </svg>
          </div>
          <p class="line-label">{{ section.label }}</p>
        </header>

        <div class="status-cards">
          <section
            v-for="status in section.statuses"
            :key="`${section.line}-${status.tone}`"
            class="status-card"
            :class="`status-card-${status.tone}`"
          >
            <h3>{{ statusLabel(status.tone) }}</h3>
            <p class="status-heading">{{ status.heading }}</p>
            <ul>
              <li v-for="item in status.body" :key="item">{{ item }}</li>
            </ul>
          </section>
        </div>
      </article>
    </section>

    <section class="guidance-panel" aria-label="Operational guidance">
      <div class="guidance-title">
        <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <path d="M8 5h8a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2Z" />
          <path d="M9.5 4h5a1 1 0 0 1 1 1v1h-7V5a1 1 0 0 1 1-1Z" />
          <path d="m9.2 11 1 1 2-2" />
          <path d="M14 11h2" />
          <path d="m9.2 15 1 1 2-2" />
          <path d="M14 15h2" />
        </svg>
        <h2>Operational Guidance</h2>
      </div>
      <ul class="guidance-list">
        <li v-for="item in operationalGuidance" :key="item">
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="m9 6 6 6-6 6" />
          </svg>
          <span>{{ item }}</span>
        </li>
      </ul>
    </section>
  </section>
</template>

<style scoped src="./MessageStatusHelpView.css"></style>
