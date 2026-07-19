import {
  type ConversationRecord,
  type MessageRecord,
} from "@reticulum/node-client";
import { defineStore } from "pinia";
import { computed, ref } from "vue";

import {
  notifyOperationalUpdateOnce,
  truncateNotificationBody,
} from "../services/operationalNotifications";
import { createProjectionClientAccessor } from "../utils/projectionClient";
import { supportsNativeNodeRuntime } from "../utils/runtimeProfile";
import {
  type ConversationListItem,
  type StoredMessages,
  canonicalConversationIdForDraft,
  chatNotificationKey,
  chatSendModeForDestination,
  cloneMessage,
  collapseConversationItems,
  conversationMatchesDestination,
  displayNameForDestination,
  draftConversationId,
  ensureDirectChatPeerConnected,
  isDraftConversationId,
  knownConversationDestinations,
  mapConversationRecord,
  messageTimestamp,
  normalizeDestinationHex,
  remoteDestinationForMessage,
  safeMessageBody,
  saveWebMessages,
} from "./messagingModel";
import { createMessagingProjectionController } from "./messagingProjection";
import { createMessagingVisualMock } from "./messagingVisualMock";
import { useNodeStore } from "./nodeStore";

const getProjectionClient = createProjectionClientAccessor("messaging");

export const useMessagingStore = defineStore("messaging", () => {
  const nodeStore = useNodeStore();
  const byMessageId = ref<StoredMessages>({});
  const nativeConversations = ref<ConversationRecord[]>([]);
  const selectedConversationId = ref<string>("");
  const selectedTargetMessageId = ref<string>("");
  const pendingConversation = ref<ConversationListItem | null>(null);
  const visualMockDeletedConversationIds = ref<Set<string>>(new Set());
  const initialized = ref(false);
  const hydrated = ref(false);
  const cleanups: Array<() => void> = [];

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
      conversationMatchesDestination(conversation, normalizedDestination, nodeStore),
    ) ?? null;
  }

  function upsertMessage(message: MessageRecord): void {
    byMessageId.value = {
      ...byMessageId.value,
      [message.messageIdHex]: cloneMessage(message),
    };
  }

  const {
    appendVisualMockOutboundMessage,
    applyVisualMockChatData,
    isVisualMockConversationDeleted,
    markVisualMockConversationDeleted,
  } = createMessagingVisualMock({
    byMessageId,
    getConversations: () => conversations.value,
    hydrated,
    nativeConversations,
    nodeStore,
    pendingConversation,
    selectedConversationId,
    selectedTargetMessageId,
    upsertMessage,
    visualMockDeletedConversationIds,
  });

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
      {
        route: "/inbox",
        conversationId: message.conversationId,
        messageIdHex: message.messageIdHex,
      },
    );
  }

  const {
    dispose,
    init,
    pendingConversationForDestination,
    refreshConversations,
    refreshMessages,
    resolvedConversationIds,
  } = createMessagingProjectionController({
    byMessageId,
    cleanups,
    findNativeConversationByDestination,
    getConversations: () => conversations.value,
    hydrated,
    initialized,
    nativeConversations,
    nodeStore,
    notifyForInboundMessage,
    pendingConversation,
    selectedConversationId,
    upsertMessage,
  });

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
    const normalizedDestination = normalizeDestinationHex(destinationHex);
    const sendMode = chatSendModeForDestination(normalizedDestination, nodeStore);
    const messageMethod = sendMode === "DirectOnly" ? "Direct" : "Opportunistic";
    if (sendMode === "DirectOnly") {
      await ensureDirectChatPeerConnected(normalizedDestination, nodeStore);
    }
    const existingConversation = findNativeConversationByDestination(normalizedDestination);
    const currentPending = pendingConversationForDestination(normalizedDestination);
    const conversationId = existingConversation?.conversationId
      ?? currentPending?.conversationId
      ?? draftConversationId(normalizedDestination);

    if (!existingConversation && !currentPending) {
      ensureConversationForDestination(normalizedDestination);
    } else if (selectedConversationId.value !== conversationId) {
      selectedConversationId.value = conversationId;
    }

    const now = Date.now();
    const optimisticMessageId = `local-${now.toString(16)}-${Math.random().toString(16).slice(2, 10)}`;
    upsertMessage({
      messageIdHex: optimisticMessageId,
      conversationId,
      direction: "Outbound",
      destinationHex: normalizedDestination,
      sourceHex: nodeStore.status.lxmfDestinationHex || undefined,
      title,
      bodyUtf8,
      method: messageMethod,
      state: "Queued",
      transportState: "Queued",
      applicationAckState: "Waiting",
      detail: undefined,
      sentAtMs: now,
      receivedAtMs: undefined,
      updatedAtMs: now,
    });
    persistWeb();

    try {
      const messageIdHex = await nodeStore.sendLxmf(normalizedDestination, bodyUtf8, title, {
        sendMode,
      });
      const nextMessages = { ...byMessageId.value };
      delete nextMessages[optimisticMessageId];
      nextMessages[messageIdHex] = cloneMessage({
        messageIdHex,
        conversationId: canonicalConversationIdForDraft(conversationId) || conversationId,
        direction: "Outbound",
        destinationHex: normalizedDestination,
        sourceHex: nodeStore.status.lxmfDestinationHex || undefined,
        title,
        bodyUtf8,
        method: messageMethod,
        state: "Queued",
        transportState: "Queued",
        applicationAckState: "Waiting",
        detail: undefined,
        sentAtMs: now,
        receivedAtMs: undefined,
        updatedAtMs: Date.now(),
      });
      byMessageId.value = nextMessages;
      persistWeb();
    } catch (error) {
      upsertMessage({
        messageIdHex: optimisticMessageId,
        conversationId,
        direction: "Outbound",
        destinationHex: normalizedDestination,
        sourceHex: nodeStore.status.lxmfDestinationHex || undefined,
        title,
        bodyUtf8,
        method: messageMethod,
        state: "Failed",
        transportState: "Failed",
        applicationAckState: "Failed",
        detail: error instanceof Error ? error.message : "Send failed",
        sentAtMs: now,
        receivedAtMs: undefined,
        updatedAtMs: Date.now(),
      });
      persistWeb();
      throw error;
    }
  }

  async function deleteConversation(conversationId: string): Promise<void> {
    const normalizedConversationId = conversationId.trim();
    if (!normalizedConversationId) {
      return;
    }

    const conversation = conversations.value.find(
      (candidate) => candidate.conversationId === normalizedConversationId,
    );
    const conversationIds = resolvedConversationIds(normalizedConversationId);
    const knownDestinations = conversation
      ? knownConversationDestinations(conversation.destinationHex, nodeStore)
      : new Set<string>();

    if (supportsNativeNodeRuntime) {
      const client = getProjectionClient(nodeStore.settings.clientMode);
      await client.deleteConversation(normalizedConversationId);
    }
    markVisualMockConversationDeleted(normalizedConversationId, conversationIds, knownDestinations);

    const nextMessages: StoredMessages = {};
    for (const message of Object.values(byMessageId.value)) {
      const messageConversationId = normalizeDestinationHex(message.conversationId);
      const messageDestination = normalizeDestinationHex(message.destinationHex);
      const messageSource = normalizeDestinationHex(message.sourceHex ?? "");
      const belongsToConversation = conversationIds.has(message.conversationId)
        || conversationIds.has(messageConversationId)
        || knownDestinations.has(messageDestination)
        || knownDestinations.has(messageSource);
      if (!belongsToConversation) {
        nextMessages[message.messageIdHex] = cloneMessage(message);
      }
    }
    byMessageId.value = nextMessages;
    nativeConversations.value = nativeConversations.value.filter((record) => {
      const conversationRecordId = normalizeDestinationHex(record.conversationId);
      const peerDestination = normalizeDestinationHex(record.peerDestinationHex);
      return !conversationIds.has(record.conversationId)
        && !conversationIds.has(conversationRecordId)
        && !knownDestinations.has(peerDestination);
    });

    if (pendingConversation.value?.conversationId === normalizedConversationId) {
      pendingConversation.value = null;
    }
    if (selectedConversationId.value === normalizedConversationId) {
      selectedConversationId.value = "";
      selectedTargetMessageId.value = "";
    }

    if (supportsNativeNodeRuntime) {
      await refreshConversations();
      if (!selectedConversationId.value) {
        selectedConversationId.value = nativeConversations.value[0]?.conversationId ?? "";
      }
      if (selectedConversationId.value) {
        await refreshMessages(selectedConversationId.value);
      }
      return;
    }

    persistWeb();
    if (!selectedConversationId.value) {
      selectedConversationId.value = conversations.value[0]?.conversationId ?? "";
    }
  }

  function selectConversation(conversationId: string): void {
    selectedConversationId.value = conversationId;
    selectedTargetMessageId.value = "";
    if (supportsNativeNodeRuntime && !isDraftConversationId(conversationId)) {
      void refreshMessages(conversationId);
    }
  }

  async function openConversationTarget(
    conversationId: string,
    messageIdHex?: string,
  ): Promise<void> {
    const normalizedConversationId = conversationId.trim();
    if (!normalizedConversationId) {
      return;
    }
    await refreshConversations();
    const matchedConversation = nativeConversations.value.find(
      (conversation) => conversation.conversationId === normalizedConversationId,
    ) ?? findNativeConversationByDestination(normalizedConversationId);

    selectedConversationId.value = matchedConversation?.conversationId ?? normalizedConversationId;
    selectedTargetMessageId.value = messageIdHex?.trim() ?? "";
    await refreshMessages(selectedConversationId.value);
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
      const nextConversations = collapseConversationItems(
        nativeConversations.value.map((record) => mapConversationRecord(record, nodeStore)),
        nodeStore,
      );
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
      const peerDestinationHex = remoteDestinationForMessage(message);
      const existing = byConversation.get(message.conversationId);
      if (existing && existing.updatedAtMs > updatedAtMs) {
        continue;
      }
      byConversation.set(message.conversationId, {
        conversationId: message.conversationId,
        destinationHex: peerDestinationHex,
        displayName: displayNameForDestination(peerDestinationHex, nodeStore),
        preview: safeMessageBody(message).slice(0, 80) || "(empty message)",
        updatedAtMs,
        state: message.state,
      });
    }

    const nextConversations = [...byConversation.values()].sort((left, right) => right.updatedAtMs - left.updatedAtMs);
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
    return messagesForConversation(conversationId);
  });

  function messagesForConversation(conversationId: string): MessageRecord[] {
    const conversationIds = resolvedConversationIds(conversationId);
    if (conversationIds.size === 0) {
      return [];
    }
    return Object.values(byMessageId.value)
      .filter((message) => conversationIds.has(message.conversationId))
      .sort((left, right) => {
        return messageTimestamp(left) - messageTimestamp(right);
      });
  }

  function messagesForDestination(destinationHex: string): MessageRecord[] {
    const knownDestinations = knownConversationDestinations(destinationHex, nodeStore);
    if (knownDestinations.size === 0) {
      return [];
    }
    return Object.values(byMessageId.value)
      .filter((message) => {
        const messageDestination = normalizeDestinationHex(message.destinationHex);
        const messageSource = normalizeDestinationHex(message.sourceHex ?? "");
        return knownDestinations.has(messageDestination) || knownDestinations.has(messageSource);
      })
      .sort((left, right) => messageTimestamp(left) - messageTimestamp(right));
  }

  return {
    initialized,
    hydrated,
    selectedConversationId,
    selectedTargetMessageId,
    conversations,
    selectedConversation,
    activeMessages,
    messagesForConversation,
    messagesForDestination,
    init,
    dispose,
    selectConversation,
    openConversationTarget,
    ensureConversationForDestination,
    applyVisualMockChatData,
    appendVisualMockOutboundMessage,
    sendMessage,
    deleteConversation,
    upsertWebMessage,
  };
});
