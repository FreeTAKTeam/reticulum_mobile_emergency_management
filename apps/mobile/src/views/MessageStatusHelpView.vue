<script setup lang="ts">
import { RouterLink } from "vue-router";

type StatusTone = "red" | "yellow" | "green" | "unknown";

interface HelpStatusBlock {
  conditions: string[];
  label: string;
  tone: StatusTone;
}

interface HelpLineSection {
  line: string;
  statuses: HelpStatusBlock[];
  summary: string;
  title: string;
}

const unknownTriggers = [
  "You lack confirmed information.",
  "Conditions are changing too rapidly to assess.",
  "You are unable to perform proper evaluation (for example, poor visibility or comms limitations).",
  "Reports are contradictory.",
  "You are relaying second-hand information without confirmation.",
];

const operationalGuidance = [
  "Always select the lowest accurate color when uncertain between two.",
  "Reassess and resend the EAM if conditions materially change.",
  "Unknown status must be resolved as soon as practical.",
  "Consistency across group members improves prioritization.",
  "Color inflation reduces credibility and response efficiency.",
];

const lineSections: HelpLineSection[] = [
  {
    line: "Line 3",
    title: "Security Status",
    summary: "Use this line to report the current threat picture around the location.",
    statuses: [
      {
        label: "Red - Threats Imminent",
        tone: "red",
        conditions: [
          "Active hostile presence observed or confirmed.",
          "Gunfire, violent activity, forced entry, or a credible immediate threat.",
          "Perimeter compromised or under active surveillance by hostile actors.",
          "Immediate defensive action required.",
        ],
      },
      {
        label: "Yellow - Not Secure but No Immediate Threat",
        tone: "yellow",
        conditions: [
          "Area unstable due to civil unrest, crime surge, or disaster impact.",
          "Suspicious activity observed but not confirmed hostile.",
          "Security perimeter incomplete or degraded.",
          "You cannot guarantee the safety of the location.",
        ],
      },
      {
        label: "Green - Secure",
        tone: "green",
        conditions: [
          "No active threats observed or reported.",
          "Controlled access to the area.",
          "Defensive posture in place.",
          "Situational awareness maintained.",
        ],
      },
      {
        label: "Unknown",
        tone: "unknown",
        conditions: [
          "No visual confirmation.",
          "No reliable reports.",
          "Environmental conditions prevent assessment.",
        ],
      },
    ],
  },
  {
    line: "Line 4",
    title: "Security Capability",
    summary: "Describe the ability to defend the position right now, not what might be available later.",
    statuses: [
      {
        label: "Red - No Defensive Capability",
        tone: "red",
        conditions: [
          "No weapons available.",
          "No trained defenders.",
          "Defensive tools are non-functional.",
          "Outnumbered beyond realistic resistance.",
        ],
      },
      {
        label: "Yellow - Limited Capability",
        tone: "yellow",
        conditions: [
          "Limited ammunition or supplies.",
          "Limited trained personnel.",
          "Equipment partially functional.",
          "Defensive capability sustainable only short-term.",
        ],
      },
      {
        label: "Green - Fully Capable",
        tone: "green",
        conditions: [
          "Weapons available and functional.",
          "Adequate ammunition.",
          "Personnel prepared and positioned.",
          "Defensive posture sustainable.",
        ],
      },
      {
        label: "Unknown",
        tone: "unknown",
        conditions: [
          "Inventory not confirmed.",
          "Personnel availability unclear.",
          "Equipment status unverified.",
        ],
      },
    ],
  },
  {
    line: "Line 5",
    title: "Preparedness (Sustainment)",
    summary: "Capture the current sustainment picture for food, water, fuel, power, and essential supplies.",
    statuses: [
      {
        label: "Red - No Sustainment Supplies",
        tone: "red",
        conditions: [
          "Food or water is insufficient for 24 hours.",
          "No fuel, power backup, or essential supplies.",
          "Immediate resupply required.",
        ],
      },
      {
        label: "Yellow - Limited Supplies",
        tone: "yellow",
        conditions: [
          "Supplies are available only for a short duration (less than one week).",
          "Rationing is required.",
          "Fuel or power is limited.",
        ],
      },
      {
        label: "Green - Adequate Supplies",
        tone: "green",
        conditions: [
          "Food, water, and power are sufficient for an extended period.",
          "Medical kits are stocked.",
          "Backup systems are operational.",
        ],
      },
      {
        label: "Unknown",
        tone: "unknown",
        conditions: [
          "Inventory has not been checked.",
          "Storage is inaccessible.",
          "Consumption rate is uncertain.",
        ],
      },
    ],
  },
  {
    line: "Line 6",
    title: "Medical Status",
    summary: "Report the most severe current medical need affecting the group.",
    statuses: [
      {
        label: "Red - Urgent Medical Need",
        tone: "red",
        conditions: [
          "Life-threatening injury.",
          "Severe bleeding.",
          "Respiratory distress.",
          "Unstable vital signs.",
          "Immediate evacuation required.",
        ],
      },
      {
        label: "Yellow - Delayed Care Acceptable",
        tone: "yellow",
        conditions: [
          "Minor fractures.",
          "Controlled bleeding.",
          "Manageable illness.",
          "Stable condition but still requires treatment.",
        ],
      },
      {
        label: "Green - No Medical Issue",
        tone: "green",
        conditions: [
          "No injuries.",
          "No medical conditions requiring intervention.",
          "All group members are stable.",
        ],
      },
      {
        label: "Unknown",
        tone: "unknown",
        conditions: [
          "Full headcount not confirmed.",
          "Individuals are unaccounted for.",
          "Medical assessment is incomplete.",
        ],
      },
    ],
  },
  {
    line: "Line 7",
    title: "Mobility Status",
    summary: "Show the best confirmed movement option available to the group right now.",
    statuses: [
      {
        label: "Red - No Movement Possible",
        tone: "red",
        conditions: [
          "Vehicle is disabled.",
          "Severe injury prevents movement.",
          "Security threat prevents relocation.",
          "Dependents cannot move safely.",
        ],
      },
      {
        label: "Yellow - Foot Movement Only",
        tone: "yellow",
        conditions: [
          "Vehicles are unavailable.",
          "Fuel is depleted.",
          "Roadways are blocked.",
          "Movement is possible but range and speed are limited.",
        ],
      },
      {
        label: "Green - Vehicular Movement Capable",
        tone: "green",
        conditions: [
          "Vehicles are operational.",
          "Adequate fuel is available.",
          "Safe travel routes are identified.",
        ],
      },
      {
        label: "Unknown",
        tone: "unknown",
        conditions: [
          "Vehicle status is unverified.",
          "Route conditions are unknown.",
          "Driver availability is uncertain.",
        ],
      },
    ],
  },
  {
    line: "Line 8",
    title: "Communications Status",
    summary: "Report communications depth and redundancy, not just whether a single radio is powered on.",
    statuses: [
      {
        label: "Red - No Alternate Communications",
        tone: "red",
        conditions: [
          "Only one communication method is available and it is failing.",
          "No radio backup.",
          "No mesh, repeater, or alternate channel.",
        ],
      },
      {
        label: "Yellow - Handheld (HT) Only",
        tone: "yellow",
        conditions: [
          "Limited to a low-power radio.",
          "Short-range capability.",
          "Battery dependent without redundancy.",
        ],
      },
      {
        label: "Green - Mobile (50W) or Better",
        tone: "green",
        conditions: [
          "High-power radio is available.",
          "Multiple communication paths exist.",
          "External antenna in use.",
          "Backup power is available.",
        ],
      },
      {
        label: "Unknown",
        tone: "unknown",
        conditions: [
          "Equipment status is not verified.",
          "Channel viability is untested.",
          "Interference is suspected but not confirmed.",
        ],
      },
    ],
  },
];
</script>

<template>
  <section class="help-view">
    <header class="help-hero">
      <RouterLink to="/messages" class="back-link">Back to Messages</RouterLink>
    </header>

    <section class="intro-grid" aria-label="General guidance">
      <article class="info-card">
        <p class="card-kicker">General Rule</p>
        <h2 class="card-title">Set the color for what is true now.</h2>
        <p class="card-body">
          A color must reflect present, verifiable conditions. Do not select a color based on
          optimism, assumption, or anticipated improvement.
        </p>
        <p class="card-body">
          If reliable information is missing, conflicting, or cannot be confirmed, select
          "Unknown." "Unknown" is not a failure. It is an explicit indicator that verification is
          required.
        </p>
      </article>

      <article class="info-card">
        <p class="card-kicker">When to Select "Unknown"</p>
        <ul class="bullet-list">
          <li v-for="item in unknownTriggers" :key="item" class="bullet-item">
            {{ item }}
          </li>
        </ul>
        <p class="card-note">
          Unknown status should trigger follow-up assessment or prioritization by receiving parties.
        </p>
      </article>
    </section>

    <section class="line-grid" aria-label="Status definitions by line">
      <article v-for="section in lineSections" :key="section.line" class="line-card">
        <header class="line-header">
          <div>
            <p class="line-label">{{ section.line }}</p>
            <h2 class="line-title">{{ section.title }}</h2>
          </div>
          <p class="line-summary">{{ section.summary }}</p>
        </header>

        <div class="status-grid">
          <section
            v-for="status in section.statuses"
            :key="`${section.line}-${status.label}`"
            class="status-card"
            :class="`status-card--${status.tone}`"
          >
            <h3 class="status-title">{{ status.label }}</h3>
            <p class="status-heading">Conditions</p>
            <ul class="bullet-list">
              <li v-for="condition in status.conditions" :key="condition" class="bullet-item">
                {{ condition }}
              </li>
            </ul>
          </section>
        </div>
      </article>
    </section>

    <section class="guidance-card" aria-label="Operational guidance">
      <p class="card-kicker">Operational Guidance</p>
      <ol class="guidance-list">
        <li v-for="item in operationalGuidance" :key="item" class="guidance-item">
          {{ item }}
        </li>
      </ol>
    </section>
  </section>
</template>

<style scoped>
.help-view {
  --help-panel: rgb(4 19 43 / 86%);
  --help-panel-strong: rgb(3 15 36 / 92%);
  --help-border: rgb(78 123 188 / 28%);
  --help-border-strong: rgb(85 179 255 / 34%);
  --help-text: #d5ecff;
  --help-muted: #9cb8db;
  --help-cyan: #8be5ff;
  display: grid;
  gap: 1rem;
  padding-bottom: 0.2rem;
}

.help-hero,
.info-card,
.line-card,
.guidance-card {
  backdrop-filter: blur(12px);
  background:
    linear-gradient(160deg, rgb(8 35 74 / 48%), transparent 42%),
    linear-gradient(180deg, var(--help-panel), var(--help-panel-strong));
  border: 1px solid var(--help-border);
  border-radius: 20px;
  box-shadow: inset 0 1px 0 rgb(113 192 255 / 10%);
}

.help-hero {
  align-items: center;
  display: flex;
  justify-content: flex-start;
  padding: 0.8rem;
}

.hero-copy {
  display: grid;
  gap: 0.55rem;
}

.hero-kicker,
.card-kicker,
.line-label {
  color: var(--help-cyan);
  font-family: var(--font-ui);
  font-size: 0.78rem;
  letter-spacing: 0.12em;
  margin: 0;
  text-transform: uppercase;
}

.hero-title,
.card-title,
.line-title {
  color: #f5fbff;
  font-family: var(--font-headline);
  letter-spacing: 0.01em;
  margin: 0;
}

.hero-title {
  font-size: clamp(1.35rem, 2.7vw, 2.45rem);
  line-height: 1.05;
}

.hero-subtitle,
.card-body,
.card-note,
.line-summary,
.status-heading,
.bullet-item,
.guidance-item {
  color: var(--help-muted);
  font-family: var(--font-body);
  margin: 0;
}

.hero-subtitle,
.card-body,
.card-note,
.line-summary {
  font-size: 0.97rem;
  line-height: 1.6;
}

.back-link {
  background: linear-gradient(110deg, #0ea6ff, #42f0ff);
  border-radius: 12px;
  color: #042a4e;
  font-family: var(--font-ui);
  font-size: 0.8rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  min-height: 2.5rem;
  padding: 0.72rem 0.95rem;
  text-decoration: none;
  text-transform: uppercase;
}

.intro-grid {
  display: grid;
  gap: 1rem;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.info-card,
.guidance-card {
  display: grid;
  gap: 0.75rem;
  padding: 1.1rem 1.15rem;
}

.card-title {
  font-size: clamp(1.05rem, 2vw, 1.5rem);
  line-height: 1.15;
}

.card-note {
  border-top: 1px solid rgb(78 123 188 / 24%);
  padding-top: 0.7rem;
}

.line-grid {
  display: grid;
  gap: 1rem;
}

.line-card {
  display: grid;
  gap: 0.95rem;
  padding: 1.1rem;
}

.line-header {
  align-items: end;
  display: grid;
  gap: 0.65rem;
  grid-template-columns: minmax(0, 1fr) minmax(240px, 420px);
}

.line-title {
  font-size: clamp(1.05rem, 2vw, 1.45rem);
}

.line-summary {
  max-width: 42ch;
  text-align: right;
}

.status-grid {
  display: grid;
  gap: 0.8rem;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.status-card {
  background: linear-gradient(180deg, rgb(10 23 51 / 96%), rgb(7 17 37 / 96%));
  border: 1px solid rgb(72 103 154 / 24%);
  border-radius: 16px;
  display: grid;
  gap: 0.55rem;
  min-height: 100%;
  padding: 0.95rem;
}

.status-card--red {
  border-color: rgb(255 112 112 / 22%);
  box-shadow: inset 0 1px 0 rgb(255 154 154 / 6%);
}

.status-card--yellow {
  border-color: rgb(255 208 103 / 20%);
  box-shadow: inset 0 1px 0 rgb(255 226 153 / 7%);
}

.status-card--green {
  border-color: rgb(102 226 166 / 22%);
  box-shadow: inset 0 1px 0 rgb(168 255 214 / 7%);
}

.status-card--unknown {
  border-color: var(--help-border-strong);
  box-shadow: inset 0 1px 0 rgb(139 229 255 / 7%);
}

.status-title {
  color: var(--help-text);
  font-family: var(--font-ui);
  font-size: 0.96rem;
  letter-spacing: 0.03em;
  margin: 0;
}

.status-heading {
  color: #7dc2f5;
  font-size: 0.78rem;
  letter-spacing: 0.09em;
  text-transform: uppercase;
}

.bullet-list,
.guidance-list {
  display: grid;
  gap: 0.45rem;
  margin: 0;
  padding-left: 1rem;
}

.bullet-item,
.guidance-item {
  font-size: 0.94rem;
  line-height: 1.45;
}

@media (max-width: 960px) {
  .help-hero,
  .line-header,
  .status-grid,
  .intro-grid {
    grid-template-columns: 1fr;
  }

  .line-summary {
    max-width: none;
    text-align: left;
  }
}

@media (max-width: 720px) {
  .help-hero,
  .info-card,
  .line-card,
  .guidance-card {
    border-radius: 16px;
  }

  .help-hero {
    align-items: stretch;
    padding: 1rem;
  }

  .back-link {
    justify-self: start;
  }

  .status-card {
    padding: 0.85rem;
  }
}
</style>
