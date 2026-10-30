<script setup lang="ts">
import type { MapMarkerSymbolRecord } from "@reticulum/node-client";
import { computed, ref } from "vue";

import { mdiPathFor } from "../../utils/mapMarkerIcons";
import { groupSymbols, markerSymbolColor, searchSymbols } from "../../utils/mapMarkerPresentation";

const props = defineProps<{
  symbols: MapMarkerSymbolRecord[];
  recent: MapMarkerSymbolRecord[];
  loading: boolean;
  error: string;
}>();

const emit = defineEmits<{
  pick: [symbol: MapMarkerSymbolRecord];
  close: [];
}>();

const query = ref("");
const matches = computed(() => searchSymbols(props.symbols, query.value));
const groups = computed(() => groupSymbols(matches.value));
const searching = computed(() => query.value.trim().length > 0);
</script>

<template>
  <section class="marker-sheet" role="dialog" aria-modal="false" aria-label="Choose a marker symbol">
    <div class="marker-sheet-head">
      <h2>Add marker</h2>
      <button type="button" class="marker-sheet-close" aria-label="Close symbol picker" @click="emit('close')">
        Cancel
      </button>
    </div>
    <input
      v-model="query"
      class="marker-symbol-search"
      type="search"
      inputmode="search"
      placeholder="Search: fire, car, hospital"
      aria-label="Search marker symbols"
    />
    <p v-if="error" class="marker-sheet-error" role="alert">{{ error }}</p>
    <p v-else-if="loading && symbols.length === 0" class="marker-sheet-note">Loading symbols…</p>

    <div class="marker-sheet-body">
      <template v-if="!searching && recent.length > 0">
        <h3>Recent</h3>
        <div class="marker-symbol-grid" data-testid="marker-recent-symbols">
          <button
            v-for="symbol in recent"
            :key="`recent-${symbol.id}`"
            type="button"
            class="marker-symbol-option"
            :data-symbol="symbol.id"
            @click="emit('pick', symbol)"
          >
            <svg :style="{ color: markerSymbolColor(symbol.id) }" viewBox="0 0 24 24" aria-hidden="true"><path :d="mdiPathFor(symbol.mdi)" fill="currentColor" /></svg>
            <span>{{ symbol.description }}</span>
          </button>
        </div>
      </template>

      <template v-for="group in groups" :key="group.category">
        <h3>{{ group.label }}</h3>
        <div class="marker-symbol-grid">
          <button
            v-for="symbol in group.symbols"
            :key="symbol.id"
            type="button"
            class="marker-symbol-option"
            :data-symbol="symbol.id"
            @click="emit('pick', symbol)"
          >
            <svg :style="{ color: markerSymbolColor(symbol.id) }" viewBox="0 0 24 24" aria-hidden="true"><path :d="mdiPathFor(symbol.mdi)" fill="currentColor" /></svg>
            <span>{{ symbol.description }}</span>
          </button>
        </div>
      </template>
      <p v-if="searching && matches.length === 0" class="marker-sheet-note">
        No symbol matches "{{ query.trim() }}".
      </p>
    </div>
  </section>
</template>

<style scoped>
.marker-sheet-body {
  max-height: min(52vh, 26rem);
  overflow-y: auto;
  overscroll-behavior: contain;
}

.marker-sheet-body h3 {
  color: #9cb3d6;
  font-family: var(--font-ui);
  font-size: 0.82rem;
  letter-spacing: 0.08em;
  margin: 0.7rem 0 0.4rem;
  text-transform: uppercase;
}

.marker-symbol-search {
  background: rgb(2 7 16 / 72%);
  border: 1px solid rgb(74 133 207 / 45%);
  border-radius: 12px;
  color: #def1ff;
  font-family: var(--font-body);
  font-size: 1rem;
  min-height: 2.9rem;
  padding: 0 0.8rem;
  width: 100%;
}

.marker-symbol-grid {
  display: grid;
  gap: 0.45rem;
  grid-template-columns: repeat(auto-fill, minmax(4.6rem, 1fr));
}

.marker-symbol-option {
  align-items: center;
  border: 1px solid var(--btn-border);
  border-radius: 12px;
  display: flex;
  flex-direction: column;
  font-family: var(--font-ui);
  font-size: 0.84rem;
  font-weight: 600;
  gap: 0.2rem;
  justify-content: center;
  min-height: 4.2rem;
  padding: 0.4rem 0.2rem;
}

.marker-symbol-option svg {
  height: 1.6rem;
  width: 1.6rem;
}

.marker-symbol-option span {
  line-height: 1.1;
  text-align: center;
}
</style>
