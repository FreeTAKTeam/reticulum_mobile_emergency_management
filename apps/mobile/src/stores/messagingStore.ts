import {
  createReticulumNodeClient,
  type ConversationRecord,
  type MessageRecord,
  type ProjectionInvalidationEvent,
  type ReticulumNodeClient,
} from "@reticulum/node-client";
import { defineStore } from "pinia";
import { computed, ref } from "vue";

import {
  notifyOperationalUpdateOnce,
  truncateNotificationBody,
} from "../services/operationalNotifications";
import { supportsNativeNodeRuntime } from "../utils/runtimeProfile";
import { useNodeStore } from "./nodeStore";

const MESSAGE_STORAGE_KEY = "reticulum.mobile.inbox.v1";

type StoredMessages = Record<string, MessageRecord>;
type ProjectionClientCache = typeof globalThis & {
  __reticulumMessagingProjectionClient?: ReticulumNodeClient;
};
type ConversationListItem = {
  conversationId: string;
  destinationHex: string;
  displayName: string;
  preview: string;
  updatedAtMs: number;
  state: string;
};

function cloneMessage(message: MessageRecord): MessageRecord {
  return {
    ...message,
    bodyUtf8: typeof message.bodyUtf8 === "string" ? message.bodyUtf8 : "",
    title: typeof message.title === "string" ? message.title : undefined,
    detail: typeof message.detail === "string" ? message.detail : undefined,
  };
}

function safeMessageBody(message: Pick<MessageRecord, "bodyUtf8">): string {
  return typeof message.bodyUtf8 === "string" ? message.bodyUtf8.trim() : "";
}

function chatNotificationKey(message: MessageRecord): string {
  return message.messageIdHex.trim().toLowerCase();
}

function loadWebMessages(): StoredMessages {
  try {
    const raw = localStorage.getItem(MESSAGE_STORAGE_KEY);
    if (!raw) {
      return {};
    }
    const parsed = JSON.parse(raw) as MessageRecord[];
    const out: StoredMessages = {};
    for (const message of parsed) {
      if (!message.messageIdHex) {
        continue;
      }
      out[message.messageIdHex] = cloneMessage(message);
    }
    return out;
  } catch {
    return {};
  }
}

function saveWebMessages(messages: StoredMessages): void {
  localStorage.setItem(MESSAGE_STORAGE_KEY, JSON.stringify(Object.values(messages)));
}

function getProjectionClient(mode: "auto" | "capacitor"): ReticulumNodeClient {
  const cache = globalThis as ProjectionClientCache;
  if (!cache.__reticulumMessagingProjectionClient) {
    cache.__reticulumMessagingProjectionClient = createReticulumNodeClient({ mode });
  }
  return cache.__reticulumMessagingProjectionClient;
}

function displayNameForDestination(
  destinationHex: string,
  nodeStore: ReturnType<typeof useNodeStore>,
): string {
  const normalized = destinationHex.trim().toLowerCase();
  const direct = nodeStore.discoveredByDestination[normalized];
  if (direct) {
    return direct.label ?? direct.announcedName ?? destinationHex;
  }
  const peer = Object.values(nodeStore.discoveredByDestination).find(
    (candidate) => candidate.lxmfDestinationHex?.trim().toLowerCase() === normalized,
  );
  return peer?.label ?? peer?.announcedName ?? destinationHex;
}

function normalizeDestinationHex(value: string): string {
  return value.trim().toLowerCase();
}

function draftConversationId(destinationHex: string): string {
  return `draft:${normalizeDestinationHex(destinationHex)}`;
}

function isDraftConversationId(value: string): boolean {
  return value.startsWith("draft:");
}

function mapConversationRecord(
  record: ConversationRecord,
  nodeStore: ReturnType<typeof useNodeStore>,
): ConversationListItem {
  const displayName = record.peerDisplayName
    ?? displayNameForDestination(record.peerDestinationHex, nodeStore);
  return {
    conversationId: record.conversationId,
    destinationHex: record.peerDestinationHex,
    displayName,
    preview: record.lastMessagePreview ?? "(empty message)",
    updatedAtMs: record.lastMessageAtMs,
    state: record.lastMessageState ?? "Queued",
  };
}

export const useMessagingStore = defineStore("messaging", () => {
  const nodeStore = useNodeStore();
  const byMessageId = ref<StoredMessages>({});
  const nativeConversations = ref<ConversationRecord[]>([]);
  const selectedConversationId = ref<string>("");
  const pendingConversation = ref<ConversationListItem | null>(null);
  const initialized = ref(false);
  const cleanups: Array<() => void> = [];

  let conversationsRefreshPromise: Promise<void> | null = null;
  let messagesRefreshPromise: Promise<void> | null = null;

  function persistWeb(): void {
    if (!supportsNativeNodeRuntime) {
      saveWebMessages(byMessageId.value);
    }
  }

  function findNativeConversationByDestination(destinationHex: string): ConversationRecord | null {
    const normalizedDestination = normalizeDestinationHex(destinationHex);
    if (!normalizedDestination) {
      return null;
    }
    return nativeConversations.value.find((conversation) =>
      normalizeDestinationHex(conversation.peerDestinationHex) === normalizedDestination,
    ) ?? null;
  }

  async function refreshConversations(): Promise<void> {
    if (!supportsNativeNodeRuntime || !nodeStore.status.running) {
      return;
    }
    if (conversationsRefreshPromise) {
      await conversationsRefreshPromise;
      return;
    }
    const promise = (async () => {
      const client = getProjectionClient(nodeStore.settings.clientMode);
      nativeConversations.value = await client.listConversations();
      const currentPending = pendingConversation.value;
      if (currentPending) {
        const matchedConversation = findNativeConversationByDestination(currentPending.destinationHex);
        if (matchedConversation) {
          if (selectedConversationId.value === currentPending.conversationId) {
            selectedConversationId.value = matchedConversation.conversationId;
          }
          pendingConversation.value = null;
        }
      }
      const currentConversationId = selectedConversationId.value.trim();
      if (!currentConversationId && nativeConversations.value.length > 0) {
        selectedConversationId.value = nativeConversations.value[0].conversationId;
      } else if (
        currentConversationId
        && !(
          pendingConversation.value
          && currentConversationId === pendingConversation.value.conversationId
        )
        && !nativeConversations.value.some((conversation) => conversation.conversationId === currentConversationId)
      ) {
        selectedConversationId.value = nativeConversations.value[0]?.conversationId ?? "";
      }
    })();
    conversationsRefreshPromise = promise;
    try {
      await promise;
    } finally {
      if (conversationsRefreshPromise === promise) {
        conversationsRefreshPromise = null;
      }
    }
  }

  async function refreshMessages(conversationId = selectedConversationId.value): Promise<void> {
    if (!supportsNativeNodeRuntime || !nodeStore.status.running) {
      return;
    }
    if (messagesRefreshPromise) {
      await messagesRefreshPromise;
      return;
    }
    const promise = (async () => {
      const client = getProjectionClient(nodeStore.settings.clientMode);
      const items = await client.listMessages(conversationId || undefined);
      const next: StoredMessages = {};
      for (const message of items) {
        next[message.messageIdHex] = cloneMessage(message);
      }
      byMessageId.value = next;
    })();
    messagesRefreshPromise = promise;
    try {
      await promise;
    } finally {
      if (messagesRefreshPromise === promise) {
        messagesRefreshPromise = null;
      }
    }
  }

  async function refreshAll(): Promise<void> {
    await refreshConversations();
    await refreshMessages();
  }

  function handleProjectionInvalidation(event: ProjectionInvalidationEvent): void {
    if (event.scope === "Conversations") {
      void refreshConversations();
      return;
    }
    if (event.scope === "Messages") {
      if (!event.key || event.key === selectedConversationId.value) {
        void refreshMessages();
      }
      void refreshConversations();
    }
  }

  function init(): void {
    if (initialized.value) {
      return;
    }
    initialized.value = true;

    if (!supportsNativeNodeRuntime) {
      byMessageId.value = loadWebMessages();
      return;
    }

    const client = getProjectionClient(nodeStore.settings.clientMode);
    cleanups.push(client.on("projectionInvalidated", handleProjectionInvalidation));
    cleanups.push(client.on("statusChanged", () => {
      void refreshAll();
    }));
    cleanups.push(client.on("messageReceived", (message) => {
      void notifyForInboundMessage(message);
    }));
    cleanups.push(client.on("messageUpdated", (message) => {
      void notifyForInboundMessage(message);
    }));
    void refreshAll();
  }

  function dispose(): void {
    while (cleanups.length > 0) {
      cleanups.pop()?.();
    }
  }

  async function notifyForInboundMessage(message: MessageRecord): Promise<void> {
    if (message.direction !== "Inbound") {
      return;
    }
    const peerHex = message.sourceHex?.trim() || message.destinationHex;
    const displayName = displayNameForDestination(peerHex, nodeStore);
    await notifyOperationalUpdateOnce(
      "chat",
      chatNotificationKey(message),
      `Message from ${displayName}`,
      truncateNotificationBody(safeMessageBody(message) || "(empty message)"),
    );
  }

  function upsertWebMessage(message: MessageRecord): void {
    byMessageId.value = {
      ...byMessageId.value,
      [message.messageIdHex]: cloneMessage(message),
    };
    persistWeb();
    if (!selectedConversationId.value && safeMessageBody(message)) {
      selectedConversationId.value = message.conversationId;
    }
  }

  async function sendMessage(destinationHex: string, bodyUtf8: string, title?: string): Promise<void> {
    nodeStore.assertReadyForOutbound("send LXMF messages");
    await nodeStore.sendLxmf(destinationHex, bodyUtf8, title);
    if (supportsNativeNodeRuntime) {
      await refreshAll();
    }
  }

  function selectConversation(conversationId: string): void {
    selectedConversationId.value = conversationId;
    if (supportsNativeNodeRuntime && !isDraftConversationId(conversationId)) {
      void refreshMessages(conversationId);
    }
  }

  function ensureConversationForDestination(destinationHex: string, displayName?: string): void {
    const normalizedDestination = normalizeDestinationHex(destinationHex);
    if (!normalizedDestination) {
      return;
    }

    const existingConversation = findNativeConversationByDestination(normalizedDestination);
    if (existingConversation) {
      pendingConversation.value = null;
      selectConversation(existingConversation.conversationId);
      return;
    }

    const nextPendingConversation: ConversationListItem = {
      conversationId: draftConversationId(normalizedDestination),
      destinationHex: normalizedDestination,
      displayName: displayName?.trim() || displayNameForDestination(normalizedDestination, nodeStore),
      preview: "New conversation",
      updatedAtMs: Date.now(),
      state: "Draft",
    };
    pendingConversation.value = nextPendingConversation;
    selectedConversationId.value = nextPendingConversation.conversationId;
  }

  const webMessages = computed(() =>
    Object.values(byMessageId.value)
      .filter((message) => safeMessageBody(message).length > 0)
      .sort((left, right) => {
        const leftTime = left.receivedAtMs ?? left.sentAtMs ?? left.updatedAtMs;
        const rightTime = right.receivedAtMs ?? right.sentAtMs ?? right.updatedAtMs;
        return leftTime - rightTime;
      }),
  );

  const conversations = computed(() => {
    if (supportsNativeNodeRuntime) {
      const nextConversations = nativeConversations.value.map((record) => mapConversationRecord(record, nodeStore));
      const currentPending = pendingConversation.value;
      if (
        currentPending
        && !nextConversations.some((conversation) =>
          normalizeDestinationHex(conversation.destinationHex)
            === normalizeDestinationHex(currentPending.destinationHex),
        )
      ) {
        return [currentPending, ...nextConversations];
      }
      return nextConversations;
    }

    const byConversation = new Map<
      string,
      ConversationListItem
    >();

    for (const message of webMessages.value.filter((candidate) => candidate.direction === "Inbound")) {
      const updatedAtMs = message.receivedAtMs ?? message.sentAtMs ?? message.updatedAtMs;
      const existing = byConversation.get(message.conversationId);
      if (existing && existing.updatedAtMs > updatedAtMs) {
        continue;
      }
      byConversation.set(message.conversationId, {
        conversationId: message.conversationId,
        destinationHex: message.destinationHex,
        displayName: displayNameForDestination(message.destinationHex, nodeStore),
        preview: safeMessageBody(message).slice(0, 80) || "(empty message)",
        updatedAtMs,
        state: message.state,
      });
    }

    return [...byConversation.values()].sort((left, right) => right.updatedAtMs - left.updatedAtMs);
  });

  const selectedConversation = computed(() =>
    conversations.value.find((conversation) => conversation.conversationId === selectedConversationId.value)
      ?? conversations.value[0]
      ?? null,
  );

  const activeMessages = computed(() => {
    const conversationId = selectedConversation.value?.conversationId ?? "";
    if (!conversationId) {
      return [];
    }
    return Object.values(byMessageId.value)
      .filter((message) => message.conversationId === conversationId)
      .sort((left, right) => {
        const leftTime = left.receivedAtMs ?? left.sentAtMs ?? left.updatedAtMs;
        const rightTime = right.receivedAtMs ?? right.sentAtMs ?? right.updatedAtMs;
        return leftTime - rightTime;
      });
  });

  return {
    initialized,
    selectedConversationId,
    conversations,
    selectedConversation,
    activeMessages,
    init,
    dispose,
    selectConversation,
    ensureConversationForDestination,
    sendMessage,
    upsertWebMessage,
  };
});
