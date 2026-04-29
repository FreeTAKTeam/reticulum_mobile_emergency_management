<script setup lang="ts">
import type {
  PluginSettingsField,
  PluginSettingsSection,
  PluginSettingsValue,
  PluginSettingsValues,
} from "../../plugins/pluginSettings";

const props = defineProps<{
  section: PluginSettingsSection;
  values: PluginSettingsValues;
}>();

const emit = defineEmits<{
  update: [pluginId: string, values: PluginSettingsValues];
  save: [pluginId: string];
}>();

function fieldValue(field: PluginSettingsField): PluginSettingsValue {
  return props.values[field.id] ?? field.defaultValue;
}

function updateField(field: PluginSettingsField, value: PluginSettingsValue): void {
  emit("update", props.section.pluginId, {
    ...props.values,
    [field.id]: value,
  });
}
</script>

<template>
  <article class="plugin-settings-card">
    <header class="plugin-settings-header">
      <div class="plugin-settings-title">
        <h3>{{ section.name }}</h3>
        <p>{{ section.pluginId }} | {{ section.version }}</p>
      </div>
      <span class="plugin-state">{{ section.state }}</span>
    </header>

    <p v-if="section.description" class="plugin-description">
      {{ section.description }}
    </p>

    <div v-if="section.fields.length > 0" class="plugin-settings-grid">
      <label
        v-for="field in section.fields"
        :key="field.id"
        class="plugin-field"
        :class="{ 'plugin-field-checkbox': field.type === 'boolean' }"
      >
        <span>{{ field.label }}</span>
        <input
          v-if="field.type === 'text'"
          type="text"
          :value="fieldValue(field)"
          :placeholder="field.placeholder"
          @input="updateField(field, ($event.target as HTMLInputElement).value)"
        />
        <input
          v-else-if="field.type === 'number'"
          type="number"
          :value="fieldValue(field)"
          :min="field.min"
          :max="field.max"
          :step="field.step"
          @input="updateField(field, Number(($event.target as HTMLInputElement).value))"
        />
        <select
          v-else-if="field.type === 'select'"
          :value="fieldValue(field)"
          @change="updateField(field, ($event.target as HTMLSelectElement).value)"
        >
          <option
            v-for="option in field.options ?? []"
            :key="option.value"
            :value="option.value"
          >
            {{ option.label }}
          </option>
        </select>
        <input
          v-else
          type="checkbox"
          :checked="Boolean(fieldValue(field))"
          @change="updateField(field, ($event.target as HTMLInputElement).checked)"
        />
      </label>
    </div>

    <p v-else class="plugin-description">This plug-in does not expose configurable fields.</p>

    <div class="plugin-actions">
      <button type="button" @click="emit('save', section.pluginId)">Save Plug-in</button>
    </div>
  </article>
</template>

<style scoped>
.plugin-settings-card {
  background: rgb(7 20 44 / 76%);
  border: 1px solid rgb(67 106 165 / 35%);
  border-radius: 12px;
  display: grid;
  gap: 0.62rem;
  padding: 0.72rem;
}

.plugin-settings-header {
  align-items: start;
  display: flex;
  gap: 0.7rem;
  justify-content: space-between;
}

.plugin-settings-title {
  display: grid;
  gap: 0.12rem;
  min-width: 0;
}

.plugin-settings-title h3,
.plugin-settings-title p,
.plugin-description {
  margin: 0;
}

.plugin-settings-title h3 {
  color: #d5eaff;
  font-family: var(--font-headline);
  font-size: 1rem;
}

.plugin-settings-title p,
.plugin-description {
  color: #8fa9d1;
  font-family: var(--font-body);
  overflow-wrap: anywhere;
}

.plugin-settings-title p {
  font-size: 0.78rem;
}

.plugin-state {
  background: rgb(13 120 195 / 34%);
  border: 1px solid rgb(95 193 255 / 42%);
  border-radius: 999px;
  color: #8fe3ff;
  flex: 0 0 auto;
  font-family: var(--font-ui);
  font-size: 0.68rem;
  letter-spacing: 0.08em;
  padding: 0.24rem 0.5rem;
  text-transform: uppercase;
}

.plugin-settings-grid {
  display: grid;
  gap: 0.6rem;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
}

.plugin-field {
  color: #a0b7db;
  display: grid;
  font-family: var(--font-body);
  font-size: 0.86rem;
  gap: 0.3rem;
}

.plugin-field-checkbox {
  align-items: center;
  grid-template-columns: 1fr auto;
}

.plugin-field input,
.plugin-field select {
  background: rgb(6 17 38 / 82%);
  border: 1px solid rgb(70 110 174 / 42%);
  border-radius: 10px;
  color: #daecff;
  font-family: var(--font-body);
  font-size: 0.95rem;
  padding: 0.48rem 0.56rem;
}

.plugin-actions {
  display: flex;
  justify-content: flex-end;
}
</style>
