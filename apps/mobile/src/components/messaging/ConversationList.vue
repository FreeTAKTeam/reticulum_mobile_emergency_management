<script setup lang="ts">
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
}>();

const emit = defineEmits<{
  select: [conversationId: string];
}>();

function hasReadablePeerName(displayName: string, destinationHex: string): boolean {
  const normalizedName = displayName.trim();
  const normalizedDestination = destinationHex.trim();
  return normalizedName.length > 0 && normalizedName.toLowerCase() !== normalizedDestination.toLowerCase();
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
      :class="{ active: item.conversationId === selectedConversationId }"
      @click="emit('select', item.conversationId)"
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
      <span class="conversation-state">{{ item.state }}</span>
    </button>
  </aside>
</template>

<style scoped>
.conversation-list {
  display: grid;
  gap: 0.55rem;
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
