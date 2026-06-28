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

<style scoped>
.mecp-help-view {
  --panel-bg: linear-gradient(155deg, rgb(6 25 55 / 92%), rgb(4 14 34 / 95%));
  --panel-border: rgb(55 148 244 / 58%);
  --cyan: #36b8ff;
  --muted: #b9cae7;
  --red: #ff6475;
  --yellow: #ffd36e;
  --green: #8df3c1;
  --unknown: #b5c7e9;
  display: grid;
  gap: 1rem;
}

.utility-row {
  display: grid;
  gap: 0.9rem;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1.1fr) minmax(0, 0.95fr);
}

.utility-chip {
  align-items: center;
  background: rgb(7 25 54 / 84%);
  border: 1px solid var(--panel-border);
  border-radius: 12px;
  box-shadow:
    inset 0 1px 0 rgb(183 235 255 / 8%),
    0 0 20px rgb(33 153 255 / 8%);
  color: #75c9ff;
  display: flex;
  font-family: var(--font-ui);
  font-size: clamp(0.9rem, 2.3vw, 1.12rem);
  font-weight: 600;
  gap: 0.7rem;
  justify-content: center;
  min-height: 3.05rem;
  min-width: 0;
  padding: 0.52rem 0.75rem;
  text-decoration: none;
}

.utility-chip svg,
.intro-icon svg,
.workflow-title svg {
  fill: none;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.utility-chip svg {
  flex: 0 0 auto;
  height: 1.22rem;
  stroke-width: 1.8;
  width: 1.22rem;
}

.utility-chip span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.filter-chip {
  justify-content: space-between;
}

.filter-chip span {
  flex: 1;
}

.chevron {
  margin-left: auto;
}

.back-chip {
  color: #7ccaff;
}

.intro-panel,
.example-panel,
.severity-panel,
.category-panel,
.workflow-panel {
  background: var(--panel-bg);
  border: 1px solid var(--panel-border);
  border-radius: 14px;
  box-shadow:
    inset 0 1px 0 rgb(190 235 255 / 7%),
    0 0 28px rgb(36 142 255 / 8%);
}

.intro-panel {
  align-items: center;
  display: grid;
  gap: 1rem;
  grid-template-columns: auto minmax(0, 1fr);
  padding: 1.05rem;
}

.intro-icon {
  align-items: center;
  background: rgb(10 39 82 / 72%);
  border: 1px solid rgb(62 163 255 / 76%);
  border-radius: 14px;
  color: var(--cyan);
  display: inline-flex;
  height: 3.25rem;
  justify-content: center;
  width: 3.25rem;
}

.intro-icon svg {
  height: 68%;
  stroke-width: 1.7;
  width: 68%;
}

.eyebrow {
  color: #64beff;
  font-family: var(--font-ui);
  font-size: 0.72rem;
  font-weight: 800;
  letter-spacing: 0.11em;
  margin: 0;
  text-transform: uppercase;
}

.intro-copy {
  display: grid;
  gap: 0.38rem;
  min-width: 0;
}

.intro-copy h2,
.section-heading h2,
.workflow-title h2 {
  color: #f5fbff;
  font-family: var(--font-headline);
  font-size: clamp(1.08rem, 2.7vw, 1.5rem);
  line-height: 1.1;
  margin: 0;
}

.intro-copy p:not(.eyebrow),
.section-heading p,
.example-card p,
.category-row p {
  color: var(--muted);
  font-family: var(--font-body);
  line-height: 1.35;
  margin: 0;
}

.example-panel,
.severity-panel,
.category-panel,
.workflow-panel {
  display: grid;
  gap: 0.78rem;
  padding: 0.9rem;
}

.panel-heading {
  align-items: center;
  border-bottom: 1px solid rgb(55 148 244 / 24%);
  display: flex;
  gap: 0.65rem;
  justify-content: space-between;
  padding-bottom: 0.75rem;
}

.code-tag,
.category-code {
  align-items: center;
  background: rgb(13 120 195 / 26%);
  border: 1px solid rgb(102 219 255 / 42%);
  border-radius: 8px;
  color: #8fe3ff;
  display: inline-flex;
  font-family: var(--font-ui);
  font-weight: 700;
  justify-content: center;
}

.code-tag {
  font-size: 0.78rem;
  letter-spacing: 0.09em;
  padding: 0.32rem 0.52rem;
  text-transform: uppercase;
}

.panel-heading strong {
  color: #7af4d3;
  font-family: var(--font-ui);
  font-size: clamp(1.28rem, 4.4vw, 2rem);
  letter-spacing: 0.04em;
}

.example-grid,
.severity-grid {
  display: grid;
  gap: 0.58rem;
  grid-template-columns: repeat(4, minmax(0, 1fr));
}

.example-card {
  background: linear-gradient(180deg, rgb(10 24 52 / 94%), rgb(5 17 39 / 94%));
  border: 1px solid rgb(73 173 255 / 34%);
  border-radius: 11px;
  display: grid;
  gap: 0.48rem;
  justify-items: center;
  min-width: 0;
  padding: 0.7rem 0.48rem;
  text-align: center;
}

.example-card > span {
  color: #7af4d3;
  font-family: var(--font-headline);
  font-size: clamp(1.12rem, 4vw, 1.6rem);
  font-weight: 800;
  line-height: 1;
}

.example-card h3,
.category-row h3 {
  color: #f4f8ff;
  font-family: var(--font-headline);
  font-size: clamp(0.76rem, 2.2vw, 1rem);
  line-height: 1.1;
  margin: 0;
}

.example-card p {
  font-size: clamp(0.64rem, 2vw, 0.82rem);
}

.section-heading {
  border-bottom: 1px solid rgb(55 148 244 / 24%);
  display: grid;
  gap: 0.22rem;
  padding-bottom: 0.7rem;
}

.severity-card {
  background: linear-gradient(180deg, rgb(10 24 52 / 94%), rgb(5 17 39 / 94%));
  border: 1px solid currentColor;
  border-radius: 11px;
  color: var(--unknown);
  display: grid;
  gap: 0.24rem;
  min-width: 0;
  padding: 0.62rem 0.42rem;
  text-align: center;
}

.severity-card-red {
  color: var(--red);
}

.severity-card-yellow {
  color: var(--yellow);
}

.severity-card-green {
  color: var(--green);
}

.severity-card-unknown {
  color: var(--unknown);
}

.severity-dot {
  background: currentColor;
  border-radius: 999px;
  box-shadow: 0 0 12px currentColor;
  height: 0.95rem;
  justify-self: center;
  width: 0.95rem;
}

.severity-card strong,
.severity-card small {
  font-family: var(--font-ui);
}

.severity-card strong {
  color: #f5fbff;
  font-size: clamp(0.74rem, 2.4vw, 0.96rem);
  line-height: 1;
}

.severity-card p {
  color: #d4e2f6;
  font-family: var(--font-body);
  font-size: clamp(0.64rem, 2vw, 0.82rem);
  line-height: 1.16;
  margin: 0;
}

.severity-card small {
  color: currentColor;
  font-size: 0.66rem;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.category-list {
  display: grid;
  gap: 0.52rem;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.category-row {
  align-items: center;
  background: rgb(8 22 50 / 82%);
  border: 1px solid rgb(75 118 185 / 44%);
  border-radius: 12px;
  display: grid;
  gap: 0.62rem;
  grid-template-columns: auto minmax(0, 1fr);
  min-width: 0;
  padding: 0.62rem;
}

.category-code {
  font-size: 0.9rem;
  height: 2.2rem;
  width: 2.2rem;
}

.category-row div {
  display: grid;
  gap: 0.16rem;
  min-width: 0;
}

.category-row p {
  font-size: clamp(0.68rem, 2vw, 0.84rem);
}

.workflow-panel {
  align-items: center;
  grid-template-columns: 8.5rem minmax(0, 1fr);
}

.workflow-title {
  align-items: center;
  border-right: 1px solid rgb(55 148 244 / 24%);
  color: #7bd3ff;
  display: grid;
  gap: 0.6rem;
  justify-items: center;
  padding-right: 1rem;
  text-align: center;
}

.workflow-title svg {
  height: 3.2rem;
  stroke-width: 1.7;
  width: 3.2rem;
}

.workflow-title h2 {
  font-size: clamp(0.95rem, 2.3vw, 1.14rem);
}

.workflow-list {
  counter-reset: workflow;
  display: grid;
  gap: 0.45rem;
  list-style: none;
  margin: 0;
  padding: 0;
}

.workflow-list li {
  align-items: center;
  border-bottom: 1px dashed rgb(95 166 238 / 24%);
  color: #bdd7f8;
  counter-increment: workflow;
  display: grid;
  font-family: var(--font-body);
  gap: 0.55rem;
  grid-template-columns: auto minmax(0, 1fr);
  line-height: 1.25;
  padding-bottom: 0.34rem;
}

.workflow-list li::before {
  align-items: center;
  background: rgb(13 120 195 / 26%);
  border: 1px solid rgb(102 219 255 / 42%);
  border-radius: 999px;
  color: #8fe3ff;
  content: counter(workflow);
  display: inline-flex;
  font-family: var(--font-ui);
  font-size: 0.72rem;
  font-weight: 700;
  height: 1.45rem;
  justify-content: center;
  width: 1.45rem;
}

@media (max-width: 720px) {
  .mecp-help-view {
    gap: 0.85rem;
    margin-inline: -0.72rem;
  }

  .utility-row {
    gap: 0.55rem;
    grid-template-columns: minmax(0, 1.1fr) minmax(0, 1.08fr) minmax(0, 0.92fr);
  }

  .utility-chip {
    font-size: clamp(0.68rem, 3.15vw, 0.86rem);
    gap: 0.32rem;
    justify-content: center;
    min-height: 2.95rem;
    padding: 0.48rem 0.32rem;
  }

  .utility-chip svg {
    height: 0.95rem;
    width: 0.95rem;
  }

  .intro-panel {
    gap: 0.62rem;
    padding: 0.72rem;
  }

  .intro-icon {
    border-radius: 10px;
    height: 2.45rem;
    width: 2.45rem;
  }

  .intro-copy h2 {
    font-size: clamp(1rem, 4.6vw, 1.32rem);
  }

  .example-panel,
  .severity-panel,
  .category-panel,
  .workflow-panel {
    border-radius: 13px;
    padding: 0.62rem;
  }

  .example-grid,
  .severity-grid {
    gap: 0.32rem;
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }

  .example-card {
    border-radius: 9px;
    gap: 0.34rem;
    padding: 0.48rem 0.28rem;
  }

  .example-card > span {
    font-size: clamp(0.96rem, 4.8vw, 1.28rem);
  }

  .example-card h3,
  .severity-card strong {
    font-size: clamp(0.58rem, 2.54vw, 0.74rem);
  }

  .example-card p,
  .severity-card p,
  .category-row p {
    font-size: clamp(0.54rem, 2.38vw, 0.68rem);
  }

  .severity-card {
    gap: 0.18rem;
    padding: 0.46rem 0.24rem;
  }

  .severity-dot {
    height: 0.76rem;
    width: 0.76rem;
  }

  .severity-card small {
    font-size: 0.54rem;
  }

  .category-list {
    gap: 0.42rem;
    grid-template-columns: 1fr;
  }

  .category-row {
    gap: 0.5rem;
    padding: 0.5rem;
  }

  .workflow-panel {
    grid-template-columns: 1fr;
  }

  .workflow-title {
    border-bottom: 1px solid rgb(55 148 244 / 24%);
    border-right: 0;
    grid-template-columns: auto minmax(0, 1fr);
    justify-items: start;
    padding: 0 0 0.75rem;
    text-align: left;
  }
}
</style>
