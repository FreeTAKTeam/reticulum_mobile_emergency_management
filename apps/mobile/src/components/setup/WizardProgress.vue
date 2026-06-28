<script setup lang="ts">
interface StepSummary {
  id: string;
  label: string;
}

const props = defineProps<{
  steps: StepSummary[];
  activeIndex: number;
}>();
</script>

<template>
  <ol class="wizard-progress" aria-label="Setup progress">
    <li
      v-for="(step, index) in props.steps"
      :key="step.id"
      class="wizard-step"
      :class="{ active: index === props.activeIndex, complete: index < props.activeIndex }"
    >
      <span class="step-index">{{ index + 1 }}</span>
      <span class="step-label">{{ step.label }}</span>
    </li>
  </ol>
</template>

<style scoped>
.wizard-progress {
  align-items: start;
  display: flex;
  gap: 0;
  justify-content: space-between;
  list-style: none;
  margin: 0;
  padding: 0.72rem 0.9rem 0.2rem;
  position: relative;
}

.wizard-progress::before {
  background: rgb(118 152 196 / 45%);
  content: "";
  height: 2px;
  left: 2.1rem;
  position: absolute;
  right: 2.1rem;
  top: 1.63rem;
}

.wizard-step {
  align-items: center;
  color: #89a5ce;
  display: grid;
  flex: 1;
  gap: 0.36rem;
  justify-items: center;
  min-width: 0;
  position: relative;
  text-align: center;
  z-index: 1;
}

.wizard-step.active {
  color: #e8f8ff;
}

.wizard-step.complete {
  color: #8cf0cf;
}

.step-index {
  align-items: center;
  background: #071025;
  border: 2px solid rgb(118 152 196 / 72%);
  border-radius: 999px;
  box-shadow: inset 0 0 0 1px rgb(1 7 16 / 78%);
  color: currentColor;
  display: inline-flex;
  font-family: var(--font-ui);
  font-size: 0.86rem;
  font-weight: 800;
  height: 2rem;
  justify-content: center;
  width: 2rem;
}

.wizard-step.active .step-index {
  background: linear-gradient(180deg, #64beff, #139fe2);
  border-color: rgb(212 247 255 / 82%);
  box-shadow:
    0 0 18px rgb(100 190 255 / 42%),
    inset 0 1px 0 rgb(255 255 255 / 42%);
  color: #03192f;
}

.wizard-step.complete .step-index {
  border-color: rgb(91 234 191 / 78%);
  color: #8cf0cf;
}

.step-label {
  font-family: var(--font-ui);
  font-size: 0.62rem;
  font-weight: 700;
  letter-spacing: 0.04em;
  line-height: 1;
  max-width: 4.4rem;
  overflow: hidden;
  text-overflow: ellipsis;
  text-transform: uppercase;
  white-space: nowrap;
}

@media (max-width: 760px) {
  .wizard-progress {
    padding-inline: 0.2rem;
  }

  .wizard-progress::before {
    left: 1.1rem;
    right: 1.1rem;
  }

  .step-label {
    display: none;
  }

  .step-index {
    font-size: 0.78rem;
    height: 1.9rem;
    width: 1.9rem;
  }
}
</style>
