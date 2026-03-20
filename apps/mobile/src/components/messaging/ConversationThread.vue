<script setup lang="ts">
import { computed, ref } from "vue";
import type { MessageRecord } from "@reticulum/node-client";

const props = defineProps<{
  destinationHex?: string;
  displayName?: string;
  messages: MessageRecord[];
}>();

const emit = defineEmits<{
  send: [bodyUtf8: string];
}>();

const draft = ref("");

const canSend = computed(() => draft.value.trim().length > 0 && Boolean(props.destinationHex));

function submit(): void {
  const bodyUtf8 = draft.value.trim();
  if (!bodyUtf8) {
    return;
  }
  emit("send", bodyUtf8);
  draft.value = "";
}
</script>

<template>
  <section class="thread">
    <header class="thread-header">
      <h2 class="thread-title">{{ displayName || "Select a conversation" }}</h2>
      <p v-if="destinationHex" class="thread-subtitle">{{ destinationHex }}</p>
    </header>

    <div class="thread-body">
      <article
        v-for="message in messages"
        :key="message.messageIdHex"
        class="bubble"
        :class="{ outbound: message.direction === 'Outbound' }"
      >
        <p v-if="message.title" class="bubble-title">{{ message.title }}</p>
        <p class="bubble-content">{{ message.bodyUtf8 }}</p>
        <div class="bubble-meta">
          <span>{{ message.state }}</span>
          <span>{{ new Date(message.receivedAtMs ?? message.sentAtMs ?? message.updatedAtMs).toLocaleTimeString() }}</span>
        </div>
      </article>
      <p v-if="messages.length === 0" class="thread-empty">
        No messages yet for this conversation.
      </p>
    </div>

    <form class="composer" @submit.prevent="submit">
      <textarea
        v-model="draft"
        class="composer-input"
        rows="4"
        placeholder="Write an LXMF message"
      />
      <button type="submit" class="composer-send" :disabled="!canSend">Send</button>
    </form>
  </section>
</template>

<style scoped>
.thread {
  display: grid;
  gap: 0.85rem;
  min-height: 0;
}

.thread-header {
  display: grid;
  gap: 0.18rem;
}

.thread-title,
.thread-subtitle,
.thread-empty,
.bubble-title,
.bubble-content,
.bubble-meta {
  margin: 0;
}

.thread-title {
  color: #f1fbff;
  font-family: var(--font-headline);
  font-size: 1.28rem;
}

.thread-subtitle,
.bubble-meta,
.thread-empty {
  color: #8ea8d1;
  font-family: var(--font-ui);
  font-size: 0.78rem;
}

.thread-body {
  display: grid;
  gap: 0.65rem;
  min-height: 18rem;
  overflow-y: auto;
}

.bubble {
  background: rgb(7 29 57 / 84%);
  border: 1px solid rgb(78 121 183 / 26%);
  border-radius: 16px 16px 16px 6px;
  display: grid;
  gap: 0.34rem;
  max-width: min(38rem, 92%);
  padding: 0.78rem 0.9rem;
}

.bubble.outbound {
  background: linear-gradient(135deg, rgb(10 74 138 / 90%), rgb(15 122 164 / 82%));
  border-color: rgb(120 227 255 / 36%);
  border-radius: 16px 16px 6px 16px;
  justify-self: end;
}

.bubble-title {
  color: #d4eeff;
  font-family: var(--font-headline);
  font-size: 0.86rem;
}

.bubble-content {
  color: #f3f9ff;
  font-family: var(--font-body);
  line-height: 1.45;
  white-space: pre-wrap;
}

.bubble-meta {
  display: flex;
  gap: 0.7rem;
  justify-content: space-between;
}

.composer {
  display: grid;
  gap: 0.65rem;
}

.composer-input {
  background: rgb(5 18 40 / 84%);
  border: 1px solid rgb(72 114 184 / 36%);
  border-radius: 14px;
  color: #dff2ff;
  font-family: var(--font-body);
  padding: 0.8rem 0.88rem;
  resize: vertical;
}

.composer-send {
  justify-self: end;
}
</style>
