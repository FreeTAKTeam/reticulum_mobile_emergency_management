<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import type { MessageRecord } from "@reticulum/node-client";

interface SosMessageMapTarget {
  incidentId: string;
  sourceHex: string;
  messageIdHex?: string;
}

const props = defineProps<{
  destinationHex?: string;
  displayName?: string;
  showBackButton?: boolean;
  targetStatus?: string;
  targetTeam?: string;
  targetLatitude?: string;
  targetLongitude?: string;
  targetMessageId?: string;
  sosMapTargets?: Record<string, SosMessageMapTarget>;
  messages: MessageRecord[];
}>();

const emit = defineEmits<{
  back: [];
  send: [bodyUtf8: string];
  viewSosOnMap: [target: SosMessageMapTarget];
}>();

const draft = ref("");
const threadBody = ref<HTMLElement | null>(null);
let lastTargetScrolled = "";

function safeTrim(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

const canSend = computed(() => draft.value.trim().length > 0 && Boolean(props.destinationHex));
const hasReadablePeerName = computed(() => {
  const displayName = safeTrim(props.displayName);
  const destinationHex = safeTrim(props.destinationHex);
  return displayName.length > 0 && displayName.toLowerCase() !== destinationHex.toLowerCase();
});
const hasTargetPosition = computed(() =>
  Boolean(safeTrim(props.targetLatitude) || safeTrim(props.targetLongitude)),
);
const visibleTargetStatus = computed(() => safeTrim(props.targetStatus) || "Unknown");
const visibleTargetTeam = computed(() => safeTrim(props.targetTeam) || "Unknown Team");

function submit(): void {
  const bodyUtf8 = draft.value.trim();
  if (!bodyUtf8) {
    return;
  }
  emit("send", bodyUtf8);
  draft.value = "";
}

function isSosMessage(message: MessageRecord): boolean {
  const detail = safeTrim(message.detail).toLowerCase();
  const body = safeTrim(message.bodyUtf8).toLowerCase();
  return detail.startsWith("sos") || body.startsWith("sos") || body.startsWith("urgence") || body.startsWith("emergency");
}

function visibleMessageBody(message: MessageRecord): string {
  const body = message.bodyUtf8;
  if (!isSosMessage(message)) {
    return body;
  }
  return body
    .split(/\r?\n/)
    .filter((line) => !safeTrim(line).toLowerCase().startsWith("gps:"))
    .join("\n")
    .trim();
}

function messageStateLabel(state: string): string {
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

function sosMapTarget(message: MessageRecord): SosMessageMapTarget | undefined {
  return props.sosMapTargets?.[message.messageIdHex.toLowerCase()];
}

function sosMapHref(message: MessageRecord): string {
  const target = sosMapTarget(message);
  if (!target) {
    return "/telemetry";
  }
  const params = new URLSearchParams({
    incident: target.incidentId,
    source: target.sourceHex,
  });
  if (target.messageIdHex) {
    params.set("message", target.messageIdHex);
  }
  return `/telemetry?${params.toString()}`;
}

function openSosOnMap(message: MessageRecord): void {
  const target = sosMapTarget(message);
  if (target) {
    emit("viewSosOnMap", target);
  }
}

function cssEscape(value: string): string {
  if (typeof CSS !== "undefined" && typeof CSS.escape === "function") {
    return CSS.escape(value);
  }
  return value.replace(/["\\]/g, "\\$&");
}

watch(
  () => [
    props.messages.length,
    props.messages[props.messages.length - 1]?.messageIdHex ?? "",
    props.targetMessageId ?? "",
  ],
  async () => {
    await nextTick();
    const body = threadBody.value;
    if (!body) {
      return;
    }

    const targetMessageId = safeTrim(props.targetMessageId);
    if (targetMessageId && targetMessageId !== lastTargetScrolled) {
      const target = body.querySelector<HTMLElement>(
        `[data-message-id="${cssEscape(targetMessageId)}"]`,
      );
      if (target) {
        target.scrollIntoView({ block: "center" });
        lastTargetScrolled = targetMessageId;
        return;
      }
    }

    body.scrollTop = body.scrollHeight;
  },
  { immediate: true },
);
</script>

<template>
  <section class="thread">
    <header v-if="displayName || destinationHex" class="target-card">
      <div class="target-card-top">
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
              <p class="target-label">Status</p>
              <p class="target-status">{{ visibleTargetStatus }}</p>
            </div>
            <div v-if="hasTargetPosition" class="target-coordinates">
              <p v-if="targetLatitude" class="target-position-value">{{ targetLatitude }}</p>
              <p v-if="targetLongitude" class="target-position-value">{{ targetLongitude }}</p>
            </div>
          </div>
        </div>
        <button
          v-if="showBackButton"
          type="button"
          class="thread-back-button"
          aria-label="Back"
          title="Back"
          @click="emit('back')"
        >
          <svg class="thread-back-icon" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M15.5 5.5 9 12l6.5 6.5" />
          </svg>
        </button>
      </div>
    </header>

    <header v-else class="thread-header">
      <h2 class="thread-title">Select a conversation</h2>
    </header>

    <section class="thread-panel">
      <div ref="threadBody" class="thread-body">
        <article
          v-for="message in messages"
          :key="message.messageIdHex"
          :data-message-id="message.messageIdHex"
          class="bubble"
          :class="{
            inbound: message.direction !== 'Outbound',
            outbound: message.direction === 'Outbound',
            sos: isSosMessage(message),
            targeted: message.messageIdHex === targetMessageId,
          }"
        >
          <span v-if="isSosMessage(message)" class="sos-badge">SOS EMERGENCY</span>
          <p v-if="message.title" class="bubble-title">{{ message.title }}</p>
          <p class="bubble-content">{{ visibleMessageBody(message) }}</p>
          <a
            v-if="isSosMessage(message) && sosMapTarget(message)"
            :href="sosMapHref(message)"
            class="sos-map-link"
            @click.prevent="openSosOnMap(message)"
          >
            Open position on telemetry map
          </a>
          <div class="bubble-meta">
            <span>{{ messageStateLabel(message.state) }}</span>
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
          rows="3"
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
  </section>
</template>

<style scoped>
.thread {
  display: grid;
  gap: 0.85rem;
  grid-template-rows: auto minmax(0, 1fr);
  height: 100%;
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
.target-coordinates,
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

.target-card-top {
  align-items: start;
  display: grid;
  gap: 0.75rem;
  grid-template-columns: minmax(0, 1fr) auto;
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

.thread-back-button {
  align-items: center;
  align-self: start;
  background: rgb(5 18 40 / 88%);
  border: 1px solid rgb(90 150 225 / 28%);
  border-radius: 10px;
  box-shadow: 0 0 14px rgb(20 240 255 / 10%);
  color: #9ee2ff;
  cursor: pointer;
  display: none;
  height: 2.2rem;
  justify-content: center;
  padding: 0;
  width: 2.2rem;
}

.thread-back-button:active {
  background: rgb(7 33 66 / 96%);
  border-color: rgb(120 227 255 / 32%);
  transform: translateY(1px) scale(0.985);
}

.thread-back-icon {
  height: 1rem;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 2.1;
  width: 1rem;
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

.target-coordinates {
  display: flex;
  flex-wrap: wrap;
  gap: 0.25rem 0.8rem;
}

.thread-subtitle,
.bubble-meta,
.thread-empty,
.target-position-value {
  color: #8ea8d1;
  font-family: var(--font-ui);
  font-size: 0.78rem;
}

.thread-panel {
  background:
    linear-gradient(180deg, rgb(8 22 48 / 92%), rgb(5 16 37 / 96%));
  border: 1px solid rgb(72 114 184 / 26%);
  border-radius: 18px;
  display: grid;
  grid-template-rows: minmax(0, 1fr) auto;
  min-width: 0;
  overflow: hidden;
}

.bubble.sos {
  background: #450a0a;
  border-color: rgb(239 68 68 / 78%);
}

.bubble.targeted {
  border-color: rgb(255 244 143 / 88%);
  box-shadow: 0 0 0 2px rgb(255 244 143 / 18%), 0 0 24px rgb(255 244 143 / 14%);
}

.sos-badge {
  align-self: start;
  background: #b91c1c;
  border-radius: 6px;
  color: #fff;
  display: inline-flex;
  font-family: var(--font-ui);
  font-size: 0.68rem;
  font-weight: 800;
  letter-spacing: 0;
  padding: 0.2rem 0.45rem;
}

.sos-map-link {
  background: transparent;
  border: 0;
  color: #fecaca;
  cursor: pointer;
  display: inline-flex;
  font-family: var(--font-ui);
  font-size: 0.78rem;
  justify-self: start;
  padding: 0;
  text-decoration: underline;
}

.thread-body {
  display: grid;
  gap: 0.65rem;
  min-height: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: 0.9rem;
}

.bubble {
  border: 1px solid transparent;
  display: grid;
  gap: 0.34rem;
  max-width: min(38rem, 92%);
  padding: 0.78rem 0.9rem;
}

.bubble.inbound {
  background: rgb(7 29 57 / 84%);
  border-color: rgb(78 121 183 / 32%);
  border-radius: 16px 16px 16px 6px;
  justify-self: start;
}

.bubble.outbound {
  background: linear-gradient(135deg, rgb(10 74 138 / 90%), rgb(15 122 164 / 82%));
  border-color: rgb(120 227 255 / 36%);
  border-radius: 16px 16px 6px 16px;
  justify-self: end;
}

.bubble.inbound .bubble-meta {
  color: #8ea8d1;
}

.bubble.outbound .bubble-meta {
  color: #b6def4;
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
  align-items: end;
  background: rgb(4 15 34 / 94%);
  border-top: 1px solid rgb(72 114 184 / 24%);
  display: grid;
  gap: 0.6rem;
  grid-template-columns: minmax(0, 1fr) auto;
  padding: 0.85rem 0.9rem 0.9rem;
}

.composer-input {
  background: rgb(5 18 40 / 84%);
  border: 1px solid rgb(72 114 184 / 36%);
  border-radius: 14px;
  color: #dff2ff;
  font-family: var(--font-body);
  min-height: 3rem;
  max-height: 7rem;
  padding: 0.8rem 0.88rem;
  resize: none;
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
  height: 2.35rem;
  justify-content: center;
  padding: 0;
  width: 2.35rem;
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
    padding: 0.85rem;
  }

  .target-card-top {
    gap: 0.65rem;
  }

  .target-card-main {
    gap: 0.7rem;
  }

  .target-avatar {
    height: 3.35rem;
    width: 3.35rem;
  }

  .target-avatar-icon {
    height: 1.45rem;
    width: 1.45rem;
  }

  .thread-title {
    font-size: 1.12rem;
  }

  .target-team {
    font-size: 0.9rem;
  }

  .thread-back-button {
    display: inline-flex;
    height: 1.95rem;
    width: 1.95rem;
  }

  .thread-back-icon {
    height: 0.92rem;
    width: 0.92rem;
  }

  .thread-panel {
    min-height: 0;
  }

  .composer {
    gap: 0.5rem;
    grid-template-columns: minmax(0, 1fr) auto;
    padding: 0.75rem;
  }

  .composer-input {
    min-height: 2.9rem;
    padding: 0.7rem 0.8rem;
  }

  .composer-send {
    height: 2.15rem;
    width: 2.15rem;
  }
}
</style>
