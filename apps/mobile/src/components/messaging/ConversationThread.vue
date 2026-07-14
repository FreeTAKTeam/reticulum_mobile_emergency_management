<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import type { MessageRecord } from "@reticulum/node-client";

interface SosMessageMapTarget {
  incidentId: string;
  sourceHex: string;
  messageIdHex?: string;
}

interface MessageBodySegment {
  type: "text" | "link";
  text: string;
  href?: string;
}

const props = defineProps<{
  destinationHex?: string;
  displayName?: string;
  showBackButton?: boolean;
  targetStatus?: string;
  targetTeam?: string;
  targetLatitude?: string;
  targetLongitude?: string;
  targetEamHref?: string;
  targetMapHref?: string;
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
const hasTargetPosition = computed(() =>
  Boolean(safeTrim(props.targetLatitude) || safeTrim(props.targetLongitude)),
);
const visibleTargetStatus = computed(() => safeTrim(props.targetStatus) || "Unknown");
const visibleTargetTeam = computed(() => safeTrim(props.targetTeam) || "Unknown Team");
const targetEamHref = computed(() => safeTrim(props.targetEamHref));
const targetMapHref = computed(() => safeTrim(props.targetMapHref));

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

function splitTrailingUrlPunctuation(value: string): { url: string; trailing: string } {
  let url = value;
  let trailing = "";
  while (url.length > 0 && /[),.!?;:]/.test(url[url.length - 1])) {
    trailing = `${url[url.length - 1]}${trailing}`;
    url = url.slice(0, -1);
  }
  return { url, trailing };
}

function linkHref(value: string): string {
  return value.toLowerCase().startsWith("www.") ? `https://${value}` : value;
}

function messageBodySegments(message: MessageRecord): MessageBodySegment[] {
  const body = visibleMessageBody(message);
  const urlPattern = /\b(?:https?:\/\/|www\.)[^\s<>"']+/gi;
  const segments: MessageBodySegment[] = [];
  let cursor = 0;
  for (let match = urlPattern.exec(body); match; match = urlPattern.exec(body)) {
    const raw = match[0];
    const { url, trailing } = splitTrailingUrlPunctuation(raw);
    if (!url) {
      continue;
    }
    if (match.index > cursor) {
      segments.push({ type: "text", text: body.slice(cursor, match.index) });
    }
    segments.push({ type: "link", text: url, href: linkHref(url) });
    if (trailing) {
      segments.push({ type: "text", text: trailing });
    }
    cursor = match.index + raw.length;
  }
  if (cursor < body.length) {
    segments.push({ type: "text", text: body.slice(cursor) });
  }
  return segments.length > 0 ? segments : [{ type: "text", text: body }];
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
          <div class="target-avatar-shell">
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
            <div class="target-avatar" aria-hidden="true">
              <svg class="target-avatar-icon" viewBox="0 0 24 24" fill="none">
                <circle cx="12" cy="8" r="3.25" />
                <path d="M5 18.25c1.9-3 4.2-4.5 7-4.5s5.1 1.5 7 4.5" />
              </svg>
            </div>
          </div>
          <div class="target-copy">
            <h2 class="thread-title">{{ displayName || destinationHex || "Select a conversation" }}</h2>
            <p class="target-team">{{ visibleTargetTeam }}</p>
            <div class="target-status-block">
              <p class="target-label">Status</p>
              <RouterLink
                v-if="targetEamHref"
                class="target-status target-detail-link sos-map-link"
                :to="targetEamHref"
                :aria-label="`Open EAM details for ${displayName || destinationHex || 'peer'}`"
                :title="`Open EAM details for ${displayName || destinationHex || 'peer'}`"
              >
                {{ visibleTargetStatus }}
              </RouterLink>
              <p v-else class="target-status">{{ visibleTargetStatus }}</p>
            </div>
            <div v-if="hasTargetPosition" class="target-coordinates">
              <RouterLink
                v-if="targetMapHref"
                class="target-position-link target-detail-link sos-map-link"
                :to="targetMapHref"
                :aria-label="`Open ${targetLatitude} ${targetLongitude} on telemetry map`"
                title="Open position on telemetry map"
              >
                <span v-if="targetLatitude" class="target-position-value">{{ targetLatitude }}</span>
                <span v-if="targetLongitude" class="target-position-value">{{ targetLongitude }}</span>
              </RouterLink>
              <template v-else>
                <p v-if="targetLatitude" class="target-position-value">{{ targetLatitude }}</p>
                <p v-if="targetLongitude" class="target-position-value">{{ targetLongitude }}</p>
              </template>
            </div>
          </div>
        </div>
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
          <p class="bubble-content">
            <template
              v-for="(segment, segmentIndex) in messageBodySegments(message)"
              :key="`${message.messageIdHex}:${segmentIndex}`"
            >
              <a
                v-if="segment.type === 'link'"
                :href="segment.href"
                class="message-link sos-map-link"
                target="_blank"
                rel="noopener noreferrer"
              >
                {{ segment.text }}
              </a>
              <span v-else>{{ segment.text }}</span>
            </template>
          </p>
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

<style scoped src="./ConversationThread.css"></style>
