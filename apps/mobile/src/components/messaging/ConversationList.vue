<script setup lang="ts">
import { shallowRef } from "vue";

interface ConversationListItem {
  conversationId: string;
  destinationHex: string;
  displayName: string;
  preview: string;
  updatedAtMs: number;
  state: string;
}

defineProps<{
  items: ConversationListItem[];
  selectedConversationId: string;
  activeSosConversationIds?: Set<string>;
}>();

const emit = defineEmits<{
  select: [conversationId: string];
  delete: [conversationId: string];
}>();

let longPressTimer: number | undefined;
const consumedLongPressConversationId = shallowRef("");

function clearLongPressTimer(): void {
  if (longPressTimer !== undefined) {
    window.clearTimeout(longPressTimer);
    longPressTimer = undefined;
  }
}

function startLongPress(conversationId: string): void {
  clearLongPressTimer();
  consumedLongPressConversationId.value = "";
  longPressTimer = window.setTimeout(() => {
    consumedLongPressConversationId.value = conversationId;
    emit("delete", conversationId);
  }, 650);
}

function handleConversationClick(conversationId: string): void {
  if (consumedLongPressConversationId.value === conversationId) {
    consumedLongPressConversationId.value = "";
    return;
  }
  emit("select", conversationId);
}

function handleContextMenu(event: MouseEvent, conversationId: string): void {
  event.preventDefault();
  clearLongPressTimer();
  emit("delete", conversationId);
}

function handleDeleteClick(event: MouseEvent, conversationId: string): void {
  event.stopPropagation();
  clearLongPressTimer();
  emit("delete", conversationId);
}

function hasReadablePeerName(displayName: string, destinationHex: string): boolean {
  const normalizedName = String(displayName ?? "").trim();
  const normalizedDestination = String(destinationHex ?? "").trim();
  return normalizedName.length > 0 && normalizedName.toLowerCase() !== normalizedDestination.toLowerCase();
}

function conversationStateLabel(state: string): string {
  if (state === "SentDirect" || state === "Delivered") {
    return "Delivered";
  }
  if (state === "SentToPropagation") {
    return "Sent to propagation";
  }
  if (state === "PathRequested") {
    return "Path requested";
  }
  if (state === "LinkEstablishing") {
    return "Link establishing";
  }
  if (state === "TimedOut") {
    return "Timed out";
  }
  return state;
}
</script>

<template>
  <aside class="conversation-list">
    <p v-if="items.length === 0" class="conversation-empty">
      No conversations yet. Discover a peer or receive an LXMF message to start a thread.
    </p>
    <article
      v-for="item in items"
      :key="item.conversationId"
      class="conversation-item"
      :class="{
        active: item.conversationId === selectedConversationId,
        sos: activeSosConversationIds?.has(item.conversationId),
      }"
      @contextmenu="handleContextMenu($event, item.conversationId)"
      @pointercancel="clearLongPressTimer"
      @pointerdown="startLongPress(item.conversationId)"
      @pointerleave="clearLongPressTimer"
      @pointerup="clearLongPressTimer"
    >
      <button
        type="button"
        class="conversation-select"
        @click="handleConversationClick(item.conversationId)"
      >
        <div class="conversation-topline">
          <p class="conversation-name">{{ item.displayName }}</p>
          <span class="conversation-time">{{ new Date(item.updatedAtMs).toLocaleTimeString() }}</span>
        </div>
        <p class="conversation-preview">{{ item.preview }}</p>
        <p
          v-if="!hasReadablePeerName(item.displayName, item.destinationHex)"
          class="conversation-destination"
        >
          {{ item.destinationHex }}
        </p>
        <span class="conversation-state">
          {{ activeSosConversationIds?.has(item.conversationId) ? "SOS ACTIVE" : conversationStateLabel(item.state) }}
        </span>
      </button>
      <button
        type="button"
        class="conversation-delete"
        :aria-label="`Delete conversation with ${item.displayName}`"
        title="Delete conversation"
        @click="handleDeleteClick($event, item.conversationId)"
        @pointerdown.stop
        @pointerup.stop
      >
        <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <path d="M3 6h18" />
          <path d="M8 6V4h8v2" />
          <path d="M6 6l1 15h10l1-15" />
          <path d="M10 10v7" />
          <path d="M14 10v7" />
        </svg>
      </button>
    </article>
  </aside>
</template>

<style scoped>
.conversation-list {
  align-content: start;
  display: grid;
  gap: 0.55rem;
  grid-auto-rows: max-content;
  min-height: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding-right: 0.1rem;
}

.conversation-empty {
  background: rgb(5 20 44 / 54%);
  border: 1px dashed rgb(73 119 184 / 28%);
  border-radius: 14px;
  color: #8ea8d1;
  font-family: var(--font-body);
  margin: 0;
  padding: 1rem;
}

.conversation-item {
  align-items: start;
  background: rgb(5 20 44 / 78%);
  border: 1px solid rgb(73 119 184 / 28%);
  border-radius: 14px;
  color: inherit;
  display: grid;
  gap: 0.3rem;
  grid-template-columns: minmax(0, 1fr) 2.2rem;
  padding: 0.82rem 0.88rem;
  text-align: left;
}

.conversation-select {
  background: transparent;
  border: 0;
  color: inherit;
  cursor: pointer;
  display: grid;
  gap: 0.3rem;
  min-width: 0;
  padding: 0;
  text-align: left;
}

.conversation-delete {
  --btn-bg: rgb(53 15 25 / 70%);
  --btn-bg-pressed: linear-gradient(180deg, rgb(199 241 255 / 96%), rgb(132 219 255 / 94%));
  --btn-border: rgb(255 100 117 / 48%);
  --btn-border-pressed: rgb(234 251 255 / 88%);
  --btn-shadow: 0 0 16px rgb(255 72 104 / 18%);
  --btn-shadow-pressed: inset 0 1px 0 rgb(255 255 255 / 75%);
  --btn-color: #ff8190;
  --btn-color-pressed: #063050;
  align-items: center;
  background: var(--btn-bg);
  border: 1px solid var(--btn-border);
  border-radius: 10px;
  box-shadow: var(--btn-shadow);
  color: var(--btn-color);
  cursor: pointer;
  display: inline-flex;
  height: 2.2rem;
  justify-content: center;
  padding: 0;
  width: 2.2rem;
}

.conversation-delete svg {
  height: 1rem;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.8;
  width: 1rem;
}

.conversation-item.active {
  border-color: rgb(104 220 255 / 72%);
  box-shadow: 0 0 0 1px rgb(104 220 255 / 18%);
}

.conversation-item.sos {
  border-color: rgb(239 68 68 / 86%);
  box-shadow: 0 0 0 1px rgb(239 68 68 / 20%);
}

.conversation-item.sos .conversation-state {
  color: #fecaca;
}

.conversation-topline {
  align-items: center;
  display: grid;
  gap: 0.6rem;
  grid-template-columns: minmax(0, 1fr) auto;
  min-width: 0;
}

.conversation-name {
  color: #ebf7ff;
  font-family: var(--font-headline);
  font-size: 1rem;
  margin: 0;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.conversation-time,
.conversation-destination,
.conversation-state,
.conversation-preview {
  margin: 0;
}

.conversation-time,
.conversation-destination,
.conversation-state {
  color: #8ea8d1;
  font-family: var(--font-ui);
  font-size: 0.74rem;
  letter-spacing: 0.05em;
}

.conversation-time {
  flex: 0 0 auto;
  white-space: nowrap;
}

.conversation-preview {
  color: #cadcf5;
  font-family: var(--font-body);
  line-height: 1.4;
}
</style>
