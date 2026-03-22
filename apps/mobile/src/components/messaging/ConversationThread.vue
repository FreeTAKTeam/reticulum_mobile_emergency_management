<script setup lang="ts">
import { computed, ref } from "vue";
import type { MessageRecord } from "@reticulum/node-client";

const props = defineProps<{
  destinationHex?: string;
  displayName?: string;
  targetStatus?: string;
  targetTeam?: string;
  targetLatitude?: string;
  targetLongitude?: string;
  messages: MessageRecord[];
}>();

const emit = defineEmits<{
  send: [bodyUtf8: string];
}>();

const draft = ref("");

const canSend = computed(() => draft.value.trim().length > 0 && Boolean(props.destinationHex));
const hasReadablePeerName = computed(() => {
  const displayName = props.displayName?.trim() ?? "";
  const destinationHex = props.destinationHex?.trim() ?? "";
  return displayName.length > 0 && displayName.toLowerCase() !== destinationHex.toLowerCase();
});
const hasTargetPosition = computed(() =>
  Boolean((props.targetLatitude?.trim() ?? "") || (props.targetLongitude?.trim() ?? "")),
);
const visibleTargetStatus = computed(() => props.targetStatus?.trim() || "Unknown");
const visibleTargetTeam = computed(() => props.targetTeam?.trim() || "Unknown Team");

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
    <header v-if="displayName || destinationHex" class="target-card">
      <div class="target-card-main">
        <div class="target-avatar" aria-hidden="true">
          <svg class="target-avatar-icon" viewBox="0 0 24 24" fill="none">
            <circle cx="12" cy="8" r="3.25" />
            <path d="M5 18.25c1.9-3 4.2-4.5 7-4.5s5.1 1.5 7 4.5" />
          </svg>
        </div>
        <div class="target-copy">
          <h2 class="thread-title">{{ displayName || destinationHex || "Select a conversation" }}</h2>
          <p class="target-team">{{ visibleTargetTeam }}</p>
          <div class="target-status-block">
            <p class="target-label">Emergency Status</p>
            <p class="target-status">{{ visibleTargetStatus }}</p>
          </div>
        </div>
      </div>
      <div v-if="hasTargetPosition" class="target-position">
        <p class="target-label">Position</p>
        <p v-if="targetLatitude" class="target-position-value">{{ targetLatitude }}</p>
        <p v-if="targetLongitude" class="target-position-value">{{ targetLongitude }}</p>
      </div>
    </header>

    <header v-else class="thread-header">
      <h2 class="thread-title">Select a conversation</h2>
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
      <button
        type="submit"
        class="composer-send"
        :aria-label="'Send message'"
        :disabled="!canSend"
        title="Send message"
      >
        <svg class="composer-send-icon" viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <path d="M4 12 20 4l-4 16-4.5-5.5z" />
          <path d="M20 4 10.5 14.5" />
        </svg>
      </button>
    </form>
  </section>
</template>

<style scoped>
.thread {
  display: grid;
  gap: 0.85rem;
  min-height: 0;
}

.thread-header,
.target-card {
  display: grid;
  gap: 0.35rem;
}

.thread-title,
.thread-subtitle,
.target-team,
.target-label,
.target-status,
.target-position-value,
.thread-empty,
.bubble-title,
.bubble-content,
.bubble-meta {
  margin: 0;
}

.target-card {
  align-items: start;
  background:
    radial-gradient(circle at 18% 22%, rgb(19 111 201 / 18%), transparent 42%),
    linear-gradient(138deg, rgb(9 24 52 / 88%), rgb(7 17 40 / 92%));
  border: 1px solid rgb(78 141 213 / 30%);
  border-radius: 16px;
  gap: 0.95rem;
  grid-template-columns: minmax(0, 1fr) auto;
  padding: 1rem;
}

.target-card-main {
  align-items: center;
  display: grid;
  gap: 0.9rem;
  grid-template-columns: auto minmax(0, 1fr);
}

.target-avatar {
  align-items: center;
  background: rgb(7 24 52 / 96%);
  border: 1px solid rgb(90 150 225 / 26%);
  border-radius: 14px;
  display: inline-flex;
  height: 4rem;
  justify-content: center;
  width: 4rem;
}

.target-avatar-icon {
  color: #9ee2ff;
  height: 1.8rem;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.8;
  width: 1.8rem;
}

.target-copy {
  display: grid;
  gap: 0.22rem;
  min-width: 0;
}

.thread-title {
  color: #f1fbff;
  font-family: var(--font-headline);
  font-size: 1.28rem;
}

.target-team {
  color: #9fcaf1;
  font-family: var(--font-body);
  font-size: 0.98rem;
}

.target-status-block {
  align-items: baseline;
  column-gap: 0.55rem;
  display: flex;
  flex-wrap: wrap;
}

.target-label {
  color: #60d8ff;
  font-family: var(--font-ui);
  font-size: 0.68rem;
  letter-spacing: 0.16em;
  text-transform: uppercase;
}

.target-status {
  color: #e6f8ff;
  font-family: var(--font-headline);
  font-size: 0.94rem;
}

.thread-subtitle,
.bubble-meta,
.thread-empty,
.target-position-value {
  color: #8ea8d1;
  font-family: var(--font-ui);
  font-size: 0.78rem;
}

.target-position {
  background: rgb(7 24 52 / 72%);
  border: 1px solid rgb(90 150 225 / 20%);
  border-radius: 14px;
  display: grid;
  gap: 0.18rem;
  min-width: 8.5rem;
  padding: 0.75rem 0.85rem;
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
  align-items: center;
  background: linear-gradient(110deg, #00a8ff, #14f0ff);
  border: 0;
  border-radius: 12px;
  box-shadow: 0 0 18px rgb(20 240 255 / 20%);
  color: #032748;
  cursor: pointer;
  display: inline-flex;
  height: 2.75rem;
  justify-content: center;
  justify-self: end;
  padding: 0;
  width: 2.75rem;
}

.composer-send:active {
  background: linear-gradient(110deg, #0678bf, #10bbd8);
  transform: translateY(1px) scale(0.985);
}

.composer-send:disabled {
  box-shadow: none;
  cursor: not-allowed;
  opacity: 0.55;
}

.composer-send-icon {
  height: 1.1rem;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.9;
  width: 1.1rem;
}

@media (max-width: 720px) {
  .target-card {
    grid-template-columns: 1fr;
  }

  .target-position {
    min-width: 0;
  }
}
</style>
