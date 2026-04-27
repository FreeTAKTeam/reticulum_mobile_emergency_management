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
  "Reassess and resend the EAM if conditions change materially.",
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
        <span>EAM Lines 3-8</span>
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

<style scoped>
.status-help-view {
  --panel-bg: linear-gradient(155deg, rgb(6 25 55 / 92%), rgb(4 14 34 / 95%));
  --panel-border: rgb(55 148 244 / 58%);
  --cyan: #36b8ff;
  --muted: #b9cae7;
  --red: #ff3f34;
  --yellow: #ffd022;
  --green: #5ff238;
  --unknown: #aeb8c9;
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

.utility-chip svg {
  flex: 0 0 auto;
  height: 1.22rem;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
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

.rule-panel,
.line-panel,
.guidance-panel {
  background: var(--panel-bg);
  border: 1px solid var(--panel-border);
  border-radius: 14px;
  box-shadow:
    inset 0 1px 0 rgb(190 235 255 / 7%),
    0 0 28px rgb(36 142 255 / 8%);
}

.rule-panel {
  align-items: center;
  display: grid;
  gap: 1rem;
  grid-template-columns: auto minmax(0, 1fr) minmax(13rem, 0.58fr);
  padding: 1.05rem;
}

.rule-icon,
.line-icon {
  align-items: center;
  background: rgb(10 39 82 / 72%);
  border: 1px solid rgb(62 163 255 / 76%);
  border-radius: 14px;
  color: var(--cyan);
  display: inline-flex;
  justify-content: center;
}

.rule-icon {
  height: 3.25rem;
  width: 3.25rem;
}

.rule-icon svg,
.line-icon svg,
.guidance-title svg {
  height: 68%;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.7;
  width: 68%;
}

.rule-copy h2,
.guidance-title h2 {
  color: #f5fbff;
  font-family: var(--font-headline);
  font-size: clamp(1.05rem, 2.7vw, 1.45rem);
  line-height: 1.1;
  margin: 0;
}

.rule-copy ul,
.status-card ul,
.guidance-list {
  margin: 0;
  padding: 0;
}

.rule-copy ul {
  color: var(--muted);
  display: grid;
  font-family: var(--font-body);
  gap: 0.28rem;
  list-style-position: inside;
  margin-top: 0.55rem;
}

.status-legend {
  border-left: 1px solid rgb(86 139 201 / 40%);
  display: grid;
  gap: 0.42rem;
  list-style: none;
  margin: 0;
  padding: 0 0 0 1.05rem;
}

.status-legend li {
  align-items: center;
  color: #ddeeff;
  display: grid;
  gap: 0.55rem;
  grid-template-columns: auto minmax(4.7rem, auto) minmax(0, 1fr);
  min-width: 0;
}

.status-legend strong {
  font-family: var(--font-ui);
  font-size: 1.04rem;
  min-width: 0;
}

.status-legend span:last-child {
  color: #e4eefc;
  font-family: var(--font-body);
  min-width: 0;
}

.legend-dot {
  background: currentColor;
  border: 2px solid currentColor;
  border-radius: 999px;
  box-shadow:
    0 0 0 2px rgb(0 0 0 / 44%) inset,
    0 0 12px currentColor;
  height: 1.1rem;
  width: 1.1rem;
}

.status-legend .legend-red {
  color: var(--red);
}

.status-legend .legend-yellow {
  color: var(--yellow);
}

.status-legend .legend-green {
  color: var(--green);
}

.status-legend .legend-unknown {
  color: var(--unknown);
}

.status-legend li span:last-child {
  color: #e4eefc;
}

.line-stack {
  display: grid;
  gap: 0.85rem;
}

.line-panel {
  display: grid;
  gap: 0.72rem;
  min-width: 0;
  padding: 0.85rem;
}

.line-title-row {
  align-items: center;
  border-bottom: 1px solid rgb(55 148 244 / 24%);
  display: flex;
  gap: 0.65rem;
  min-width: 0;
  padding-bottom: 0.75rem;
}

.line-label {
  color: #7bd3ff;
  font-family: var(--font-ui);
  font-weight: 700;
  letter-spacing: 0.06em;
  margin: 0;
  text-transform: uppercase;
}

.line-icon {
  flex: 0 0 auto;
  height: 2.55rem;
  width: 2.55rem;
}

.line-label {
  color: #dcecff;
  font-size: 1rem;
  line-height: 1;
  text-transform: none;
}

.status-cards {
  display: grid;
  align-items: stretch;
  gap: 0.58rem;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  min-width: 0;
}

.status-card {
  background: linear-gradient(180deg, rgb(10 24 52 / 94%), rgb(5 17 39 / 94%));
  border: 1px solid currentColor;
  border-radius: 11px;
  color: var(--unknown);
  display: grid;
  grid-template-rows: auto auto auto;
  height: 100%;
  min-width: 0;
  overflow: hidden;
}

.status-card h3 {
  border-bottom: 1px solid currentColor;
  color: currentColor;
  font-family: var(--font-headline);
  font-size: clamp(0.85rem, 2.4vw, 1.08rem);
  line-height: 1;
  margin: 0;
  padding: 0.62rem 0.48rem 0.48rem;
  text-align: center;
  white-space: nowrap;
}

.status-heading {
  color: #f4f8ff;
  font-family: var(--font-headline);
  font-size: clamp(0.74rem, 2.2vw, 0.98rem);
  font-weight: 700;
  line-height: 1.14;
  margin: 0;
  padding: 0.56rem 0.52rem 0.16rem;
  text-align: center;
  overflow-wrap: break-word;
}

.status-card ul {
  display: grid;
  gap: 0.24rem;
  list-style: none;
  padding: 0.46rem 0.62rem 0.6rem;
}

.status-card li {
  color: #d4e2f6;
  font-family: var(--font-body);
  font-size: clamp(0.68rem, 2vw, 0.86rem);
  line-height: 1.22;
  overflow-wrap: break-word;
  padding-left: 0.72rem;
  position: relative;
}

.status-card li::before {
  color: currentColor;
  content: ">";
  font-family: var(--font-ui);
  font-weight: 700;
  left: 0;
  position: absolute;
  top: 0;
}

.status-card-red {
  color: var(--red);
}

.status-card-yellow {
  color: var(--yellow);
}

.status-card-green {
  color: var(--green);
}

.status-card-unknown {
  color: var(--unknown);
}

.guidance-panel {
  align-items: center;
  display: grid;
  gap: 1.1rem;
  grid-template-columns: 9rem minmax(0, 1fr);
  padding: 1rem;
}

.guidance-title {
  align-items: center;
  border-right: 1px solid rgb(55 148 244 / 24%);
  color: #7bd3ff;
  display: grid;
  gap: 0.6rem;
  justify-items: center;
  padding-right: 1.05rem;
  text-align: center;
}

.guidance-title svg {
  height: 3.4rem;
  width: 3.4rem;
}

.guidance-title h2 {
  font-size: clamp(0.95rem, 2.3vw, 1.14rem);
}

.guidance-list {
  display: grid;
  gap: 0.45rem;
  list-style: none;
}

.guidance-list li {
  align-items: start;
  border-bottom: 1px dashed rgb(95 166 238 / 24%);
  color: #bdd7f8;
  display: grid;
  font-family: var(--font-body);
  font-size: clamp(0.78rem, 2vw, 0.95rem);
  gap: 0.55rem;
  grid-template-columns: auto minmax(0, 1fr);
  line-height: 1.25;
  padding-bottom: 0.34rem;
}

.guidance-list svg {
  color: #78d2ff;
  height: 1rem;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 2;
  width: 1rem;
}

@media (max-width: 900px) {
  .rule-panel {
    align-items: start;
    grid-template-columns: auto minmax(0, 1fr);
  }

  .status-legend {
    border-left: 0;
    border-top: 1px solid rgb(86 139 201 / 40%);
    grid-column: 1 / -1;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    padding: 0.85rem 0 0;
  }
}

@media (max-width: 720px) {
  .status-help-view {
    gap: 0.85rem;
    margin-inline: -0.72rem;
  }

  .utility-row {
    gap: 0.55rem;
    grid-template-columns: minmax(0, 1.28fr) minmax(0, 1.08fr) minmax(0, 0.92fr);
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

  .filter-chip {
    gap: 0.24rem;
  }

  .status-legend {
    gap: 0.5rem 0.42rem;
  }

  .status-legend li {
    gap: 0.32rem;
    grid-template-columns: auto minmax(0, 3.55rem) minmax(0, 1fr);
  }

  .status-legend strong,
  .status-legend span:last-child {
    font-size: 0.84rem;
    line-height: 1.05;
  }

  .legend-dot {
    height: 0.9rem;
    width: 0.9rem;
  }

  .line-panel {
    gap: 0.46rem;
    padding: 0.56rem;
  }

  .line-title-row {
    gap: 0.42rem;
    padding-bottom: 0.42rem;
  }

  .line-icon {
    border-radius: 9px;
    height: 1.8rem;
    width: 1.8rem;
  }

  .line-label {
    font-size: 0.88rem;
    line-height: 1;
  }

  .status-cards {
    gap: 0.3rem;
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }

  .status-card {
    border-radius: 9px;
  }

  .status-card h3 {
    font-size: clamp(0.62rem, 2.78vw, 0.78rem);
    padding: 0.44rem 0.18rem 0.34rem;
  }

  .status-heading {
    font-size: clamp(0.58rem, 2.54vw, 0.72rem);
    padding: 0.4rem 0.22rem 0.08rem;
  }

  .status-card ul {
    gap: 0.18rem;
    padding: 0.34rem 0.32rem 0.42rem;
  }

  .status-card li {
    font-size: clamp(0.55rem, 2.38vw, 0.68rem);
    line-height: 1.16;
    padding-left: 0.5rem;
  }

  .rule-panel,
  .guidance-panel {
    border-radius: 13px;
  }

  .guidance-panel {
    grid-template-columns: 1fr;
  }

  .guidance-title {
    border-bottom: 1px solid rgb(55 148 244 / 24%);
    border-right: 0;
    grid-template-columns: auto minmax(0, 1fr);
    justify-items: start;
    padding: 0 0 0.75rem;
    text-align: left;
  }
}
</style>
