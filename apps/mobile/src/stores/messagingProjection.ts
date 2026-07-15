import {
  type ConversationRecord,
  type MessageRecord,
  type ProjectionInvalidationEvent,
} from "@reticulum/node-client";
import type { Ref } from "vue";

import { primeOperationalNotificationScope } from "../services/operationalNotifications";
import { projectionRefreshCoordinator } from "../utils/projectionRefreshCoordinator";
import { createProjectionClientAccessor } from "../utils/projectionClient";
import { supportsNativeNodeRuntime } from "../utils/runtimeProfile";
import {
  type ConversationListItem,
  type StoredMessages,
  canonicalConversationIdForDraft,
  chatNotificationKey,
  cloneMessage,
  conversationMatchesDestination,
  isDraftConversationId,
  knownConversationDestinations,
  loadWebMessages,
  messageTimestamp,
  normalizeDestinationHex,
  safeMessageBody,
} from "./messagingModel";
import { useNodeStore } from "./nodeStore";

const getProjectionClient = createProjectionClientAccessor("messaging");

interface MessagingProjectionContext {
  byMessageId: Ref<StoredMessages>;
  cleanups: Array<() => void>;
  findNativeConversationByDestination: (destinationHex: string) => ConversationRecord | null;
  getConversations: () => ConversationListItem[];
  hydrated: Ref<boolean>;
  initialized: Ref<boolean>;
  nativeConversations: Ref<ConversationRecord[]>;
  nodeStore: ReturnType<typeof useNodeStore>;
  notifyForInboundMessage: (message: MessageRecord) => Promise<void>;
  pendingConversation: Ref<ConversationListItem | null>;
  selectedConversationId: Ref<string>;
  upsertMessage: (message: MessageRecord) => void;
}

export function createMessagingProjectionController(context: MessagingProjectionContext) {
  const {
    byMessageId,
    cleanups,
    findNativeConversationByDestination,
    getConversations,
    hydrated,
    initialized,
    nativeConversations,
    nodeStore,
    notifyForInboundMessage,
    pendingConversation,
    selectedConversationId,
    upsertMessage,
  } = context;
  let initPromise: Promise<void> | null = null;

  function mergeFetchedMessages(items: MessageRecord[]): void {
    const fetchedMessages = items.map((message) => cloneMessage(message));

    const next: StoredMessages = {};
    for (const message of Object.values(byMessageId.value)) {
      next[message.messageIdHex] = cloneMessage(message);
    }

    for (const message of fetchedMessages) {
      next[message.messageIdHex] = message;
    }
    byMessageId.value = next;
  }

  function pendingConversationForDestination(destinationHex: string): ConversationListItem | null {
    const currentPending = pendingConversation.value;
    if (!currentPending) {
      return null;
    }
    return normalizeDestinationHex(currentPending.destinationHex) === normalizeDestinationHex(destinationHex)
      ? currentPending
      : null;
  }

  function resolvedConversationIds(conversationId: string): Set<string> {
    const ids = new Set<string>();
    const normalizedConversationId = conversationId.trim();
    if (!normalizedConversationId) {
      return ids;
    }
    ids.add(normalizedConversationId);
    const canonicalConversationId = canonicalConversationIdForDraft(normalizedConversationId);
    if (canonicalConversationId) {
      ids.add(canonicalConversationId);
    }
    const matchedListConversation = getConversations().find((conversation) =>
      normalizeDestinationHex(conversation.conversationId) === normalizeDestinationHex(normalizedConversationId)
      || normalizeDestinationHex(conversation.destinationHex) === normalizeDestinationHex(normalizedConversationId),
    );
    if (matchedListConversation) {
      ids.add(matchedListConversation.conversationId);
      for (const alias of knownConversationDestinations(matchedListConversation.destinationHex, nodeStore)) {
        ids.add(alias);
      }
    }
    if (!isDraftConversationId(normalizedConversationId)) {
      return ids;
    }
    const currentPending = pendingConversation.value;
    if (!currentPending || currentPending.conversationId !== normalizedConversationId) {
      return ids;
    }
    const matchedConversation = findNativeConversationByDestination(currentPending.destinationHex);
    if (matchedConversation) {
      ids.add(matchedConversation.conversationId);
    }
    return ids;
  }

  function matchingNativeConversationForDraft(
    conversationId: string,
  ): ConversationRecord | null {
    const canonicalConversationId = canonicalConversationIdForDraft(conversationId);
    if (!canonicalConversationId) {
      return null;
    }

    return nativeConversations.value.find((conversation) =>
      conversation.conversationId === canonicalConversationId
      || conversationMatchesDestination(conversation, canonicalConversationId, nodeStore),
    ) ?? null;
  }

  function pendingConversationMatchesMessage(message: MessageRecord): boolean {
    const currentPending = pendingConversation.value;
    if (!currentPending) {
      return false;
    }

    const knownDestinations = knownConversationDestinations(currentPending.destinationHex, nodeStore);
    const messageDestination = normalizeDestinationHex(message.destinationHex);
    const messageSource = normalizeDestinationHex(message.sourceHex ?? "");
    const messageConversationId = normalizeDestinationHex(message.conversationId);

    return knownDestinations.has(messageDestination)
      || knownDestinations.has(messageSource)
      || knownDestinations.has(messageConversationId);
  }

  function adoptCanonicalConversationFromMessage(message: MessageRecord): void {
    if (!pendingConversationMatchesMessage(message)) {
      return;
    }

    const currentPending = pendingConversation.value;
    if (!currentPending) {
      return;
    }

    const nextConversationId = message.conversationId.trim()
      || canonicalConversationIdForDraft(currentPending.conversationId);
    if (!nextConversationId) {
      return;
    }

    const previousConversationId = currentPending.conversationId;
    pendingConversation.value = {
      ...currentPending,
      conversationId: nextConversationId,
      preview: safeMessageBody(message) || currentPending.preview,
      updatedAtMs: messageTimestamp(message),
      state: message.state,
    };

    if (selectedConversationId.value === previousConversationId) {
      selectedConversationId.value = nextConversationId;
    }
  }

  function resolvePendingConversationFromNativeConversation(
    conversation: ConversationRecord | null,
  ): void {
    const currentPending = pendingConversation.value;
    if (!currentPending || !conversation) {
      return;
    }
    if (!conversationMatchesDestination(conversation, currentPending.destinationHex, nodeStore)) {
      return;
    }
    if (selectedConversationId.value === currentPending.conversationId) {
      selectedConversationId.value = conversation.conversationId;
    }
    pendingConversation.value = null;
  }

  async function syncConversationStateForMessage(message: MessageRecord): Promise<void> {
    upsertMessage(message);
    adoptCanonicalConversationFromMessage(message);
    await refreshConversations();

    const matchedConversation = nativeConversations.value.find((conversation) =>
      conversation.conversationId === message.conversationId,
    ) ?? findNativeConversationByDestination(message.destinationHex)
      ?? findNativeConversationByDestination(message.sourceHex ?? "");

    resolvePendingConversationFromNativeConversation(matchedConversation);

    if (matchedConversation) {
      if (
        !selectedConversationId.value
        || (
          isDraftConversationId(selectedConversationId.value)
          && pendingConversationForDestination(matchedConversation.peerDestinationHex)
        )
      ) {
        selectedConversationId.value = matchedConversation.conversationId;
      }
    }

    if (
      selectedConversationId.value === message.conversationId
      || matchedConversation?.conversationId === selectedConversationId.value
    ) {
      await refreshMessages(message.conversationId);
    }
  }

  async function refreshConversations(): Promise<void> {
    if (!supportsNativeNodeRuntime) {
      return;
    }
    await projectionRefreshCoordinator.run("chat:conversations", async () => {
      const client = getProjectionClient(nodeStore.settings.clientMode);
      nativeConversations.value = await client.listConversations();
      const currentPending = pendingConversation.value;
      if (currentPending) {
        const matchedConversation = findNativeConversationByDestination(currentPending.destinationHex);
        resolvePendingConversationFromNativeConversation(matchedConversation);
      }
      const currentConversationId = selectedConversationId.value.trim();
      const matchedDraftConversation = isDraftConversationId(currentConversationId)
        ? matchingNativeConversationForDraft(currentConversationId)
        : null;
      if (matchedDraftConversation && selectedConversationId.value === currentConversationId) {
        selectedConversationId.value = matchedDraftConversation.conversationId;
      }
      if (!currentConversationId && nativeConversations.value.length > 0) {
        selectedConversationId.value = nativeConversations.value[0].conversationId;
      } else if (
        currentConversationId
        && !(
          pendingConversation.value
          && currentConversationId === pendingConversation.value.conversationId
        )
        && !nativeConversations.value.some(
          (conversation) => conversation.conversationId === currentConversationId,
        )
      ) {
        selectedConversationId.value = nativeConversations.value[0]?.conversationId ?? "";
      }
    }, { trailing: true });
  }

  async function refreshMessages(conversationId = selectedConversationId.value): Promise<void> {
    if (!supportsNativeNodeRuntime) {
      return;
    }
    const requestedConversationId = conversationId.trim();
    await projectionRefreshCoordinator.run("chat:messages", async () => {
      const client = getProjectionClient(nodeStore.settings.clientMode);
      let resolvedConversationId = requestedConversationId;
      if (!resolvedConversationId && selectedConversationId.value) {
        resolvedConversationId = selectedConversationId.value.trim();
      }
      if (isDraftConversationId(resolvedConversationId)) {
        const matchedConversation = pendingConversation.value
          ? findNativeConversationByDestination(pendingConversation.value.destinationHex)
          : matchingNativeConversationForDraft(resolvedConversationId);
        if (matchedConversation) {
          resolvedConversationId = matchedConversation.conversationId;
          resolvePendingConversationFromNativeConversation(matchedConversation);
        } else {
          resolvedConversationId = canonicalConversationIdForDraft(resolvedConversationId);
        }
      }
      const items = await client.listMessages(resolvedConversationId || undefined);
      mergeFetchedMessages(items);
    }, { trailing: Boolean(requestedConversationId) });
  }

  async function refreshAll(): Promise<void> {
    await refreshConversations();
    await refreshMessages();
  }

  async function hydrateStartupHistory(): Promise<void> {
    if (!supportsNativeNodeRuntime) {
      byMessageId.value = loadWebMessages();
      hydrated.value = true;
      return;
    }

    const client = getProjectionClient(nodeStore.settings.clientMode);
    await refreshConversations();
    const items = await client.listMessages(undefined);
    mergeFetchedMessages(items);
    primeOperationalNotificationScope(
      "chat",
      items
        .filter((message) => message.direction === "Inbound")
        .map((message) => chatNotificationKey(message)),
    );
    if (!selectedConversationId.value && nativeConversations.value.length > 0) {
      selectedConversationId.value = nativeConversations.value[0].conversationId;
    }
    if (selectedConversationId.value) {
      await refreshMessages(selectedConversationId.value);
    }
    hydrated.value = true;
  }

  function handleProjectionInvalidation(event: ProjectionInvalidationEvent): void {
    if (event.scope === "Conversations") {
      void refreshConversations();
      return;
    }
    if (event.scope === "Messages") {
      void refreshMessages();
      void refreshConversations();
    }
  }

  async function init(): Promise<void> {
    if (initPromise) {
      return initPromise;
    }
    if (initialized.value) {
      return;
    }

    initPromise = (async () => {
      initialized.value = true;

      if (!supportsNativeNodeRuntime) {
        await hydrateStartupHistory();
        return;
      }

      const client = getProjectionClient(nodeStore.settings.clientMode);
      cleanups.push(client.on("projectionInvalidated", handleProjectionInvalidation));
      cleanups.push(client.on("statusChanged", () => {
        void refreshAll();
      }));
      cleanups.push(client.on("messageReceived", (message) => {
        void syncConversationStateForMessage(message);
        void notifyForInboundMessage(message);
      }));
      cleanups.push(client.on("messageUpdated", (message) => {
        void syncConversationStateForMessage(message);
        void notifyForInboundMessage(message);
      }));
      await hydrateStartupHistory();
    })().finally(() => {
      initPromise = null;
    });

    return initPromise;
  }

  function dispose(): void {
    while (cleanups.length > 0) {
      cleanups.pop()?.();
    }
  }

  return {
    dispose,
    hydrateStartupHistory,
    init,
    pendingConversationForDestination,
    refreshAll,
    refreshConversations,
    refreshMessages,
    resolvedConversationIds,
    syncConversationStateForMessage,
  };
}
