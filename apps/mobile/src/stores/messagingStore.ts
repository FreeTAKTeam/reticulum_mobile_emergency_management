import { defineStore } from "pinia";
import { computed, ref } from "vue";
import type { MessageRecord } from "@reticulum/node-client";

import { useNodeStore } from "./nodeStore";

const MESSAGE_STORAGE_KEY = "reticulum.mobile.inbox.v1";

type StoredMessages = Record<string, MessageRecord>;

function cloneMessage(message: MessageRecord): MessageRecord {
  return { ...message };
}

function loadStoredMessages(): StoredMessages {
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

function saveStoredMessages(messages: StoredMessages): void {
  localStorage.setItem(MESSAGE_STORAGE_KEY, JSON.stringify(Object.values(messages)));
}

function displayNameForDestination(destinationHex: string, nodeStore: ReturnType<typeof useNodeStore>): string {
  const direct = nodeStore.discoveredByDestination[destinationHex];
  if (direct) {
    return direct.label ?? direct.announcedName ?? destinationHex;
  }
  const peer = Object.values(nodeStore.discoveredByDestination).find(
    (candidate) => candidate.lxmfDestinationHex === destinationHex,
  );
  return peer?.label ?? peer?.announcedName ?? destinationHex;
}

function isVisibleChatMessage(message: MessageRecord): boolean {
  return message.bodyUtf8.trim().length > 0;
}

export const useMessagingStore = defineStore("messaging", () => {
  const nodeStore = useNodeStore();
  const byMessageId = ref<StoredMessages>({});
  const selectedConversationId = ref<string>("");
  const initialized = ref(false);
  let unsubscribe: (() => void) | null = null;

  function persist(): void {
    saveStoredMessages(byMessageId.value);
  }

  function upsertMessage(message: MessageRecord): void {
    byMessageId.value = {
      ...byMessageId.value,
      [message.messageIdHex]: cloneMessage(message),
    };
    persist();
    if (!selectedConversationId.value && isVisibleChatMessage(message)) {
      selectedConversationId.value = message.conversationId;
    }
  }

  function init(): void {
    if (initialized.value) {
      return;
    }
    initialized.value = true;
    byMessageId.value = loadStoredMessages();
    unsubscribe = nodeStore.onMessage((message) => {
      upsertMessage(message);
    });
  }

  function dispose(): void {
    unsubscribe?.();
    unsubscribe = null;
  }

  async function sendMessage(destinationHex: string, bodyUtf8: string, title?: string): Promise<void> {
    nodeStore.assertReadyForOutbound("send LXMF messages");
    await nodeStore.sendLxmf(destinationHex, bodyUtf8, title);
  }

  function selectConversation(conversationId: string): void {
    selectedConversationId.value = conversationId;
  }

  const messages = computed(() =>
    Object.values(byMessageId.value)
      .filter((message) => isVisibleChatMessage(message))
      .sort((left, right) => {
        const leftTime = left.receivedAtMs ?? left.sentAtMs ?? left.updatedAtMs;
        const rightTime = right.receivedAtMs ?? right.sentAtMs ?? right.updatedAtMs;
        return leftTime - rightTime;
      }),
  );

  const conversations = computed(() => {
    const byConversation = new Map<
      string,
      {
        conversationId: string;
        destinationHex: string;
        displayName: string;
        preview: string;
        updatedAtMs: number;
        state: MessageRecord["state"];
      }
    >();

    for (const message of messages.value) {
      const updatedAtMs = message.receivedAtMs ?? message.sentAtMs ?? message.updatedAtMs;
      const existing = byConversation.get(message.conversationId);
      if (existing && existing.updatedAtMs > updatedAtMs) {
        continue;
      }
      byConversation.set(message.conversationId, {
        conversationId: message.conversationId,
        destinationHex: message.destinationHex,
        displayName: displayNameForDestination(message.destinationHex, nodeStore),
        preview: message.bodyUtf8.trim().slice(0, 80) || "(empty message)",
        updatedAtMs,
        state: message.state,
      });
    }

    for (const peer of nodeStore.discoveredPeers) {
      const destinationHex = peer.lxmfDestinationHex ?? peer.destination;
      if (!destinationHex) {
        continue;
      }
      const conversationId = destinationHex;
      if (byConversation.has(conversationId)) {
        continue;
      }
      byConversation.set(conversationId, {
        conversationId,
        destinationHex,
        displayName: peer.label ?? peer.announcedName ?? destinationHex,
        preview: peer.state === "connected" ? "Ready to message" : "Peer discovered via announce",
        updatedAtMs: peer.lastSeenAt,
        state: "Queued",
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
    return messages.value.filter((message) => message.conversationId === conversationId);
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
    sendMessage,
  };
});
