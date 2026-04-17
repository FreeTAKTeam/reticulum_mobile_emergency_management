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
    <button
      v-for="item in items"
      :key="item.conversationId"
      type="button"
      class="conversation-item"
      :class="{
        active: item.conversationId === selectedConversationId,
        sos: activeSosConversationIds?.has(item.conversationId),
      }"
      @click="handleConversationClick(item.conversationId)"
      @contextmenu="handleContextMenu($event, item.conversationId)"
      @pointercancel="clearLongPressTimer"
      @pointerdown="startLongPress(item.conversationId)"
      @pointerleave="clearLongPressTimer"
      @pointerup="clearLongPressTimer"
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
  background: rgb(5 20 44 / 78%);
  border: 1px solid rgb(73 119 184 / 28%);
  border-radius: 14px;
  color: inherit;
  cursor: pointer;
  display: grid;
  gap: 0.3rem;
  padding: 0.82rem 0.88rem;
  text-align: left;
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
  display: flex;
  gap: 0.6rem;
  justify-content: space-between;
}

.conversation-name {
  color: #ebf7ff;
  font-family: var(--font-headline);
  font-size: 1rem;
  margin: 0;
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

.conversation-preview {
  color: #cadcf5;
  font-family: var(--font-body);
  line-height: 1.4;
}
</style>
