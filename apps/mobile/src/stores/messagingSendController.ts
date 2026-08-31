import {
  type ConversationRecord,
  type MessageRecord,
} from "@reticulum/node-client";
import type { Ref } from "vue";

import {
  type ConversationListItem,
  type StoredMessages,
  canonicalConversationIdForDraft,
  chatSendModeForDestination,
  cloneMessage,
  draftConversationId,
  ensureDirectChatPeerConnected,
  hasInboundReplyHistory,
  isLocalChatMessageId,
  isRetryableChatMessage,
  normalizeDestinationHex,
} from "./messagingModel";
import { useNodeStore } from "./nodeStore";

interface MessagingSendContext {
  byMessageId: Ref<StoredMessages>;
  ensureConversationForDestination: (destinationHex: string) => void;
  findNativeConversationByDestination: (destinationHex: string) => ConversationRecord | null;
  nodeStore: ReturnType<typeof useNodeStore>;
  pendingConversationForDestination: (destinationHex: string) => ConversationListItem | null;
  persistWeb: () => void;
  selectedConversationId: Ref<string>;
  upsertMessage: (message: MessageRecord) => void;
}

export function createMessagingSendController(context: MessagingSendContext) {
  const {
    byMessageId,
    ensureConversationForDestination,
    findNativeConversationByDestination,
    nodeStore,
    pendingConversationForDestination,
    persistWeb,
    selectedConversationId,
    upsertMessage,
  } = context;

  function outboundMessageRecord(
    messageIdHex: string,
    conversationId: string,
    destinationHex: string,
    bodyUtf8: string,
    title: string | undefined,
    method: MessageRecord["method"],
    state: MessageRecord["state"],
    detail: string | undefined,
    sentAtMs: number,
    trafficClass: MessageRecord["trafficClass"] = "chat",
  ): MessageRecord {
    const failed = state === "Failed" || state === "TimedOut";
    return {
      messageIdHex,
      conversationId,
      direction: "Outbound",
      destinationHex,
      sourceHex: nodeStore.status.lxmfDestinationHex || undefined,
      title,
      bodyUtf8,
      trafficClass,
      method,
      state,
      transportState: failed ? "Failed" : "Queued",
      applicationAckState: failed ? "Failed" : "Waiting",
      detail,
      sentAtMs,
      receivedAtMs: undefined,
      updatedAtMs: Date.now(),
    };
  }

  function inboundReplyAllowed(destinationHex: string): boolean {
    return hasInboundReplyHistory(
      destinationHex,
      Object.values(byMessageId.value),
      nodeStore,
    );
  }

  async function dispatchLocalMessage(messageIdHex: string): Promise<void> {
    const message = byMessageId.value[messageIdHex];
    if (!message || !isLocalChatMessageId(messageIdHex)) {
      return;
    }
    const normalizedDestination = normalizeDestinationHex(message.destinationHex);
    const isInboundReply = inboundReplyAllowed(normalizedDestination);
    const sendMode = chatSendModeForDestination(
      normalizedDestination,
      nodeStore,
      isInboundReply,
    );
    const method = sendMode === "DirectOnly" ? "Direct" : "Opportunistic";
    upsertMessage(outboundMessageRecord(
      messageIdHex,
      message.conversationId,
      normalizedDestination,
      message.bodyUtf8,
      message.title,
      method,
      "Queued",
      undefined,
      message.sentAtMs ?? Date.now(),
      message.trafficClass,
    ));
    persistWeb();

    try {
      nodeStore.assertReadyForOutbound("send LXMF messages");
      if (sendMode === "DirectOnly" && !isInboundReply) {
        await ensureDirectChatPeerConnected(normalizedDestination, nodeStore);
      }
      const nativeMessageIdHex = await nodeStore.sendLxmf(
        normalizedDestination,
        message.bodyUtf8,
        message.title,
        { sendMode },
      );
      const nextMessages = { ...byMessageId.value };
      delete nextMessages[messageIdHex];
      nextMessages[nativeMessageIdHex] = cloneMessage(outboundMessageRecord(
        nativeMessageIdHex,
        canonicalConversationIdForDraft(message.conversationId) || message.conversationId,
        normalizedDestination,
        message.bodyUtf8,
        message.title,
        method,
        "Queued",
        undefined,
        message.sentAtMs ?? Date.now(),
      ));
      byMessageId.value = nextMessages;
      persistWeb();
    } catch (error) {
      upsertMessage(outboundMessageRecord(
        messageIdHex,
        message.conversationId,
        normalizedDestination,
        message.bodyUtf8,
        message.title,
        method,
        "Failed",
        error instanceof Error ? error.message : "Send failed",
        message.sentAtMs ?? Date.now(),
      ));
      persistWeb();
    }
  }

  async function sendMessage(destinationHex: string, bodyUtf8: string, title?: string): Promise<void> {
    const normalizedDestination = normalizeDestinationHex(destinationHex);
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
    upsertMessage(outboundMessageRecord(
      optimisticMessageId,
      conversationId,
      normalizedDestination,
      bodyUtf8,
      title,
      "Opportunistic",
      "Queued",
      undefined,
      now,
    ));
    persistWeb();
    await dispatchLocalMessage(optimisticMessageId);
  }

  async function retryMessage(messageIdHex: string): Promise<void> {
    const message = byMessageId.value[messageIdHex];
    if (!message || !isRetryableChatMessage(message)) {
      return;
    }
    if (isLocalChatMessageId(messageIdHex)) {
      await dispatchLocalMessage(messageIdHex);
      return;
    }

    upsertMessage(outboundMessageRecord(
      message.messageIdHex,
      message.conversationId,
      message.destinationHex,
      message.bodyUtf8,
      message.title,
      message.method,
      "Queued",
      undefined,
      message.sentAtMs ?? Date.now(),
      message.trafficClass,
    ));
    persistWeb();
    try {
      await nodeStore.retryLxmf(messageIdHex);
    } catch (error) {
      upsertMessage(outboundMessageRecord(
        message.messageIdHex,
        message.conversationId,
        message.destinationHex,
        message.bodyUtf8,
        message.title,
        message.method,
        "Failed",
        error instanceof Error ? error.message : "Retry failed",
        message.sentAtMs ?? Date.now(),
        message.trafficClass,
      ));
      persistWeb();
    }
  }

  return { retryMessage, sendMessage };
}
