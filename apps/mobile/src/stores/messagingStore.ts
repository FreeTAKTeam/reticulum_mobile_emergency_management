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
  primeOperationalNotificationScope,
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
    (candidate) =>
      candidate.lxmfDestinationHex?.trim().toLowerCase() === normalized
      || candidate.identityHex?.trim().toLowerCase() === normalized,
  );
  return peer?.label ?? peer?.announcedName ?? destinationHex;
}

function normalizeDestinationHex(value: string): string {
  return value.trim().toLowerCase();
}

function peerForDestination(
  destinationHex: string,
  nodeStore: ReturnType<typeof useNodeStore>,
) {
  const normalized = normalizeDestinationHex(destinationHex);
  if (!normalized) {
    return null;
  }
  return nodeStore.discoveredByDestination[normalized]
    ?? Object.values(nodeStore.discoveredByDestination).find((candidate) =>
      normalizeDestinationHex(candidate.destination) === normalized
      || normalizeDestinationHex(candidate.lxmfDestinationHex ?? "") === normalized
      || normalizeDestinationHex(candidate.identityHex ?? "") === normalized,
    )
    ?? null;
}

function knownConversationDestinations(
  destinationHex: string,
  nodeStore: ReturnType<typeof useNodeStore>,
): Set<string> {
  const known = new Set<string>();
  const normalized = normalizeDestinationHex(destinationHex);
  if (!normalized) {
    return known;
  }

  known.add(normalized);
  const peer = peerForDestination(normalized, nodeStore);
  if (!peer) {
    return known;
  }

  known.add(normalizeDestinationHex(peer.destination));
  if (peer.lxmfDestinationHex) {
    known.add(normalizeDestinationHex(peer.lxmfDestinationHex));
  }
  if (peer.identityHex) {
    known.add(normalizeDestinationHex(peer.identityHex));
  }
  return known;
}

function conversationAliasKey(
  conversation: ConversationListItem,
  nodeStore: ReturnType<typeof useNodeStore>,
): string {
  const aliases = knownConversationDestinations(conversation.destinationHex, nodeStore);
  const normalizedConversationId = normalizeDestinationHex(conversation.conversationId);
  if (normalizedConversationId) {
    aliases.add(normalizedConversationId);
  }
  return [...aliases].sort()[0] ?? normalizedConversationId;
}

function collapseConversationItems(
  conversations: ConversationListItem[],
  nodeStore: ReturnType<typeof useNodeStore>,
): ConversationListItem[] {
  const byPeer = new Map<string, ConversationListItem>();
  for (const conversation of conversations) {
    const key = conversationAliasKey(conversation, nodeStore);
    const existing = byPeer.get(key);
    if (!existing || existing.updatedAtMs <= conversation.updatedAtMs) {
      byPeer.set(key, conversation);
    }
  }
  return [...byPeer.values()].sort((left, right) => right.updatedAtMs - left.updatedAtMs);
}

function conversationMatchesDestination(
  conversation: Pick<ConversationRecord, "peerDestinationHex">,
  destinationHex: string,
  nodeStore: ReturnType<typeof useNodeStore>,
): boolean {
  const conversationDestination = normalizeDestinationHex(conversation.peerDestinationHex);
  if (!conversationDestination) {
    return false;
  }
  return knownConversationDestinations(destinationHex, nodeStore).has(conversationDestination);
}

function draftConversationId(destinationHex: string): string {
  return `draft:${normalizeDestinationHex(destinationHex)}`;
}

function isDraftConversationId(value: string): boolean {
  return value.startsWith("draft:");
}

function canonicalConversationIdForDraft(conversationId: string): string {
  if (!isDraftConversationId(conversationId)) {
    return conversationId.trim();
  }
  return normalizeDestinationHex(conversationId.slice("draft:".length));
}

function messageTimestamp(message: MessageRecord): number {
  return message.receivedAtMs ?? message.sentAtMs ?? message.updatedAtMs;
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
  const selectedTargetMessageId = ref<string>("");
  const pendingConversation = ref<ConversationListItem | null>(null);
  const initialized = ref(false);
  const hydrated = ref(false);
  const cleanups: Array<() => void> = [];

  let initPromise: Promise<void> | null = null;
  let conversationsRefreshPromise: Promise<void> | null = null;
  let messagesRefreshPromise: Promise<void> | null = null;
  let conversationsRefreshQueued = false;
  let queuedMessagesConversationId: string | null = null;

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

  function conversationSetForRefresh(conversationId: string): Set<string> {
    const normalizedConversationId = conversationId.trim();
    if (!normalizedConversationId) {
      return new Set();
    }
    return resolvedConversationIds(normalizedConversationId);
  }

  function mergeFetchedMessages(
    requestedConversationId: string,
    items: MessageRecord[],
  ): void {
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
    const matchedListConversation = conversations.value.find((conversation) =>
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
    if (conversationsRefreshPromise) {
      conversationsRefreshQueued = true;
      await conversationsRefreshPromise;
      return;
    }
    const promise = (async () => {
      const client = getProjectionClient(nodeStore.settings.clientMode);
      do {
        conversationsRefreshQueued = false;
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
      } while (conversationsRefreshQueued);
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
    if (!supportsNativeNodeRuntime) {
      return;
    }
    const requestedConversationId = conversationId.trim();
    if (messagesRefreshPromise) {
      queuedMessagesConversationId = requestedConversationId;
      await messagesRefreshPromise;
      return;
    }
    const promise = (async () => {
      const client = getProjectionClient(nodeStore.settings.clientMode);
      let nextConversationId = requestedConversationId;
      do {
        queuedMessagesConversationId = null;
        let resolvedConversationId = nextConversationId;
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
        mergeFetchedMessages(resolvedConversationId, items);
        nextConversationId = queuedMessagesConversationId ?? "";
      } while (nextConversationId);
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

  async function hydrateStartupHistory(): Promise<void> {
    if (!supportsNativeNodeRuntime) {
      byMessageId.value = loadWebMessages();
      hydrated.value = true;
      return;
    }

    const client = getProjectionClient(nodeStore.settings.clientMode);
    await refreshConversations();
    const items = await client.listMessages(undefined);
    mergeFetchedMessages("", items);
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
      method: "Direct",
      state: "Queued",
      detail: undefined,
      sentAtMs: now,
      receivedAtMs: undefined,
      updatedAtMs: now,
    });
    persistWeb();

    try {
      const messageIdHex = await nodeStore.sendLxmf(normalizedDestination, bodyUtf8, title);
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
        method: "Direct",
        state: "Queued",
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
        method: "Direct",
        state: "Failed",
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
    hydrateStartupHistory,
    ensureConversationForDestination,
    sendMessage,
    deleteConversation,
    upsertWebMessage,
  };
});
