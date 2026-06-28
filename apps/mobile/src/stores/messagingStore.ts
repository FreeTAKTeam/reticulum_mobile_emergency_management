import {
  createReticulumNodeClient,
  type ConversationRecord,
  type MessageRecord,
  type ProjectionInvalidationEvent,
  type ReticulumNodeClient,
  type SendMode,
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
const DIRECT_CHAT_CONNECT_TIMEOUT_MS = 7_000;
const DIRECT_CHAT_CONNECT_POLL_MS = 250;

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

function normalizeDestinationHex(value: string): string {
  return value.trim().toLowerCase();
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

function isDestinationHash(value: string | undefined): boolean {
  return typeof value === "string" && /^[0-9a-f]{32}$/i.test(value.trim());
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

function peerDisplayName(
  peer: ReturnType<typeof peerForDestination>,
): string | undefined {
  return peer?.announcedName?.trim() || peer?.label?.trim() || undefined;
}

function displayNameForDestination(
  destinationHex: string,
  nodeStore: ReturnType<typeof useNodeStore>,
): string {
  const normalized = normalizeDestinationHex(destinationHex);
  const peer = peerForDestination(normalized, nodeStore);
  const displayName = peerDisplayName(peer);
  if (displayName) {
    return displayName;
  }
  return nodeStore.savedByDestination[normalized]?.label?.trim() || destinationHex;
}

function activePeerForDestination(
  destinationHex: string,
  nodeStore: ReturnType<typeof useNodeStore>,
): boolean {
  return peerForDestination(destinationHex, nodeStore)?.activeLink === true;
}

function savedPeerRouteForDestination(
  destinationHex: string,
  nodeStore: ReturnType<typeof useNodeStore>,
) {
  const normalized = normalizeDestinationHex(destinationHex);
  const peer = peerForDestination(normalized, nodeStore);
  const candidates = [
    normalized,
    peer?.destination,
    peer?.lxmfDestinationHex,
    peer?.identityHex,
  ]
    .map((candidate) => normalizeDestinationHex(candidate ?? ""))
    .filter(isDestinationHash);
  for (const candidate of candidates) {
    const saved = nodeStore.savedByDestination[candidate];
    if (saved) {
      return saved;
    }
  }
  return null;
}

function hasKnownLxmfRoute(
  destinationHex: string,
  nodeStore: ReturnType<typeof useNodeStore>,
): boolean {
  const peer = peerForDestination(destinationHex, nodeStore);
  const saved = savedPeerRouteForDestination(destinationHex, nodeStore);
  return isDestinationHash(peer?.lxmfDestinationHex)
    || isDestinationHash(saved?.lxmfDestinationHex)
    || isDestinationHash(peer?.destination)
    || isDestinationHash(saved?.destination);
}

function canUseRelayChat(
  destinationHex: string,
  nodeStore: ReturnType<typeof useNodeStore>,
): boolean {
  return Boolean(nodeStore.bestPropagationNodeHex)
    && Boolean(savedPeerRouteForDestination(destinationHex, nodeStore))
    && hasKnownLxmfRoute(destinationHex, nodeStore);
}

function chatSendModeForDestination(
  destinationHex: string,
  nodeStore: ReturnType<typeof useNodeStore>,
): SendMode {
  return canUseRelayChat(destinationHex, nodeStore) ? "Auto" : "DirectOnly";
}

function peerConnectionDestination(
  destinationHex: string,
  nodeStore: ReturnType<typeof useNodeStore>,
): string {
  const normalized = normalizeDestinationHex(destinationHex);
  const peer = peerForDestination(normalized, nodeStore);
  return normalizeDestinationHex(peer?.destination ?? normalized);
}

async function waitForDirectPeerLink(
  destinationHex: string,
  nodeStore: ReturnType<typeof useNodeStore>,
): Promise<boolean> {
  const deadline = Date.now() + DIRECT_CHAT_CONNECT_TIMEOUT_MS;
  do {
    if (activePeerForDestination(destinationHex, nodeStore)) {
      return true;
    }
    await sleep(DIRECT_CHAT_CONNECT_POLL_MS);
  } while (Date.now() < deadline);
  return activePeerForDestination(destinationHex, nodeStore);
}

async function ensureDirectChatPeerConnected(
  destinationHex: string,
  nodeStore: ReturnType<typeof useNodeStore>,
): Promise<void> {
  if (activePeerForDestination(destinationHex, nodeStore)) {
    return;
  }

  const connectionDestination = peerConnectionDestination(destinationHex, nodeStore);
  await nodeStore.connectPeer(connectionDestination);
  if (await waitForDirectPeerLink(destinationHex, nodeStore)) {
    return;
  }

  const displayName = displayNameForDestination(destinationHex, nodeStore);
  throw new Error(
    `Peer ${displayName} is not connected and no propagation relay route is available. Announce, connect, or wait for a relay before sending chat.`,
  );
}

function remoteDestinationForMessage(message: MessageRecord): string {
  if (message.direction === "Inbound") {
    return normalizeDestinationHex(message.sourceHex ?? "") || normalizeDestinationHex(message.destinationHex);
  }
  return normalizeDestinationHex(message.destinationHex);
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
  const resolvedDisplayName = displayNameForDestination(record.peerDestinationHex, nodeStore);
  const nativeDisplayName = record.peerDisplayName?.trim();
  const displayName = nativeDisplayName && !isDestinationHash(nativeDisplayName)
    ? nativeDisplayName
    : resolvedDisplayName;
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
  const visualMockDeletedConversationIds = ref<Set<string>>(new Set());
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
        mergeFetchedMessages(items);
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

  function markVisualMockConversationDeleted(
    conversationId: string,
    conversationIds: Set<string>,
    knownDestinations: Set<string>,
  ): void {
    if (!import.meta.env.DEV) {
      return;
    }
    const next = new Set(visualMockDeletedConversationIds.value);
    for (const id of conversationIds) {
      next.add(normalizeDestinationHex(id));
    }
    for (const destination of knownDestinations) {
      next.add(normalizeDestinationHex(destination));
    }
    next.add(normalizeDestinationHex(conversationId));
    visualMockDeletedConversationIds.value = next;
  }

  function isVisualMockConversationDeleted(value: string | undefined): boolean {
    const normalized = normalizeDestinationHex(value ?? "");
    return Boolean(normalized && visualMockDeletedConversationIds.value.has(normalized));
  }

  function messageBelongsToDeletedVisualMockConversation(message: MessageRecord): boolean {
    return isVisualMockConversationDeleted(message.conversationId)
      || isVisualMockConversationDeleted(message.destinationHex)
      || isVisualMockConversationDeleted(message.sourceHex);
  }

  function applyVisualMockChatData(): void {
    if (!import.meta.env.DEV) {
      return;
    }

    const now = Date.now();
    const localLxmfDestination = nodeStore.status.lxmfDestinationHex || "00000000000000000000000000000001";
    const peerRecords = [
      {
        destination: "a13f6e2b94cd08ff31a92765db4e10c2",
        identityHex: "fdd5d08e476a4602bc51d0f37d72dd21",
        lxmfDestinationHex: "3ac7e918b5f1407bb759b0f3f4d41c9a",
        label: "Field Team Alpha",
        announcedName: "ALPHA-1",
        activeLink: true,
      },
      {
        destination: "c974de6aa1f8417a8c2e0bb5332ac01f",
        identityHex: "31397ec9c46d4caea5739f50821cecd7",
        lxmfDestinationHex: "18a738f903344a11a8c56695454da331",
        label: "North checkpoint",
        announcedName: "NORTH-CP",
        activeLink: false,
      },
      {
        destination: "f08ad9c21be64737a5bb68fd4434e912",
        identityHex: "e1b68f14e71d4cde8629ffbc5471459b",
        lxmfDestinationHex: "9b8fe7dc314446438d4ceab380208f6a",
        label: "Medical triage",
        announcedName: "TRIAGE-2",
        activeLink: true,
      },
    ];

    for (const [index, peer] of peerRecords.entries()) {
      const lastSeenAt = now - (index + 1) * 70_000;
      nodeStore.discoveredByDestination[peer.destination] = {
        destination: peer.destination,
        identityHex: peer.identityHex,
        lxmfDestinationHex: peer.lxmfDestinationHex,
        announceLastSeenAt: lastSeenAt,
        lxmfLastSeenAt: lastSeenAt,
        label: peer.label,
        announcedName: peer.announcedName,
        appData: "R3AKT,EmergencyMessages,Telemetry,LXMF",
        hops: index + 1,
        interfaceHex: `mockchat000000000${index}`,
        lastSeenAt,
        sources: ["announce", "import"],
        state: peer.activeLink ? "connected" : "disconnected",
        saved: true,
        stale: false,
        activeLink: peer.activeLink,
      };
      nodeStore.savedByDestination[peer.destination] = {
        destination: peer.destination,
        label: peer.label,
        savedAt: now - (index + 2) * 60 * 60_000,
      };
    }

    const conversations: ConversationRecord[] = [
      {
        conversationId: peerRecords[0].lxmfDestinationHex,
        peerDestinationHex: peerRecords[0].lxmfDestinationHex,
        peerDisplayName: "ALPHA-1",
        lastMessagePreview: "Copy. Two operators moving to the north entrance now.",
        lastMessageAtMs: now - 45_000,
        unreadCount: 0,
        lastMessageState: "Delivered",
      },
      {
        conversationId: peerRecords[2].lxmfDestinationHex,
        peerDestinationHex: peerRecords[2].lxmfDestinationHex,
        peerDisplayName: "TRIAGE-2",
        lastMessagePreview: "Emergency: patient transport requested at triage tent.",
        lastMessageAtMs: now - 4 * 60_000,
        unreadCount: 1,
        lastMessageState: "Received",
      },
      {
        conversationId: peerRecords[1].lxmfDestinationHex,
        peerDestinationHex: peerRecords[1].lxmfDestinationHex,
        peerDisplayName: "NORTH-CP",
        lastMessagePreview: "Route update queued through propagation.",
        lastMessageAtMs: now - 18 * 60_000,
        unreadCount: 0,
        lastMessageState: "SentToPropagation",
      },
    ];

    const composedMockMessages = Object.values(byMessageId.value)
      .filter((message) => message.messageIdHex.startsWith("900000000000000000000000"))
      .filter((message) => !messageBelongsToDeletedVisualMockConversation(message))
      .map((message) => cloneMessage(message));
    const messages: MessageRecord[] = [
      {
        messageIdHex: "10000000000000000000000000000001",
        conversationId: peerRecords[0].lxmfDestinationHex,
        direction: "Inbound",
        destinationHex: peerRecords[0].lxmfDestinationHex,
        sourceHex: peerRecords[0].lxmfDestinationHex,
        title: "Status check",
        bodyUtf8: "Alpha team is staged at checkpoint A. Radio relay is stable.",
        method: "Direct",
        state: "Received",
        receivedAtMs: now - 9 * 60_000,
        updatedAtMs: now - 9 * 60_000,
      },
      {
        messageIdHex: "10000000000000000000000000000002",
        conversationId: peerRecords[0].lxmfDestinationHex,
        direction: "Outbound",
        destinationHex: peerRecords[0].lxmfDestinationHex,
        sourceHex: localLxmfDestination,
        bodyUtf8: "Send two operators to the north entrance and confirm when they are in position.",
        method: "Direct",
        state: "Delivered",
        sentAtMs: now - 3 * 60_000,
        updatedAtMs: now - 2 * 60_000,
      },
      {
        messageIdHex: "10000000000000000000000000000003",
        conversationId: peerRecords[0].lxmfDestinationHex,
        direction: "Inbound",
        destinationHex: peerRecords[0].lxmfDestinationHex,
        sourceHex: peerRecords[0].lxmfDestinationHex,
        bodyUtf8: "Copy. Two operators moving to the north entrance now.",
        method: "Direct",
        state: "Received",
        receivedAtMs: now - 45_000,
        updatedAtMs: now - 45_000,
      },
      {
        messageIdHex: "20000000000000000000000000000001",
        conversationId: peerRecords[2].lxmfDestinationHex,
        direction: "Inbound",
        destinationHex: peerRecords[2].lxmfDestinationHex,
        sourceHex: peerRecords[2].lxmfDestinationHex,
        title: "Medical",
        bodyUtf8: "Emergency: patient transport requested at triage tent.\nGPS: 46.81,-71.20",
        method: "Direct",
        state: "Received",
        detail: "SOS priority message",
        receivedAtMs: now - 4 * 60_000,
        updatedAtMs: now - 4 * 60_000,
      },
      {
        messageIdHex: "20000000000000000000000000000002",
        conversationId: peerRecords[2].lxmfDestinationHex,
        direction: "Outbound",
        destinationHex: peerRecords[2].lxmfDestinationHex,
        sourceHex: localLxmfDestination,
        bodyUtf8: "Transport team notified. Keep the patient at the marked triage point.",
        method: "Direct",
        state: "SentDirect",
        sentAtMs: now - 2 * 60_000,
        updatedAtMs: now - 2 * 60_000,
      },
      {
        messageIdHex: "30000000000000000000000000000001",
        conversationId: peerRecords[1].lxmfDestinationHex,
        direction: "Inbound",
        destinationHex: peerRecords[1].lxmfDestinationHex,
        sourceHex: peerRecords[1].lxmfDestinationHex,
        bodyUtf8: "North checkpoint is intermittent. Propagation relay is preferred until the link is stable.",
        method: "Propagated",
        state: "Received",
        receivedAtMs: now - 21 * 60_000,
        updatedAtMs: now - 21 * 60_000,
      },
      {
        messageIdHex: "30000000000000000000000000000002",
        conversationId: peerRecords[1].lxmfDestinationHex,
        direction: "Outbound",
        destinationHex: peerRecords[1].lxmfDestinationHex,
        sourceHex: localLxmfDestination,
        title: "Route update",
        bodyUtf8: "Route update queued through propagation. Confirm when NORTH-CP comes back online.",
        method: "Propagated",
        state: "SentToPropagation",
        detail: "Using propagation relay fallback",
        sentAtMs: now - 18 * 60_000,
        updatedAtMs: now - 17 * 60_000,
      },
      ...composedMockMessages,
    ];

    const visibleConversations = conversations.filter((conversation) =>
      !isVisualMockConversationDeleted(conversation.conversationId)
        && !isVisualMockConversationDeleted(conversation.peerDestinationHex),
    );
    const visibleMessages = messages.filter((message) =>
      !messageBelongsToDeletedVisualMockConversation(message),
    );

    nativeConversations.value = visibleConversations;
    byMessageId.value = Object.fromEntries(visibleMessages.map((message) => [message.messageIdHex, message]));
    pendingConversation.value = null;
    if (
      !selectedConversationId.value
      || !visibleConversations.some((item) => item.conversationId === selectedConversationId.value)
    ) {
      selectedConversationId.value = visibleConversations[0]?.conversationId ?? "";
    }
    selectedTargetMessageId.value = isVisualMockConversationDeleted(peerRecords[2].lxmfDestinationHex)
      ? ""
      : "20000000000000000000000000000001";
    hydrated.value = true;
  }

  function appendVisualMockOutboundMessage(destinationHex: string, bodyUtf8: string): void {
    if (!import.meta.env.DEV) {
      return;
    }
    const normalizedDestination = normalizeDestinationHex(destinationHex);
    const conversation = conversations.value.find((item) =>
      normalizeDestinationHex(item.destinationHex) === normalizedDestination
      || normalizeDestinationHex(item.conversationId) === normalizedDestination,
    );
    const conversationId = conversation?.conversationId ?? normalizedDestination;
    const now = Date.now();
    const message: MessageRecord = {
      messageIdHex: `900000000000000000000000${now.toString(16).slice(-8)}`,
      conversationId,
      direction: "Outbound",
      destinationHex: conversation?.destinationHex ?? normalizedDestination,
      sourceHex: nodeStore.status.lxmfDestinationHex || undefined,
      bodyUtf8,
      method: "Opportunistic",
      state: "Queued",
      sentAtMs: now,
      updatedAtMs: now,
    };
    upsertMessage(message);
    const nativeConversation = nativeConversations.value.find((item) => item.conversationId === conversationId);
    if (nativeConversation) {
      nativeConversation.lastMessagePreview = bodyUtf8.slice(0, 80) || "(empty message)";
      nativeConversation.lastMessageAtMs = now;
      nativeConversation.lastMessageState = "Queued";
    }
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
    hydrateStartupHistory,
    ensureConversationForDestination,
    applyVisualMockChatData,
    appendVisualMockOutboundMessage,
    sendMessage,
    deleteConversation,
    upsertWebMessage,
  };
});
