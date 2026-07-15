import {
  type ConversationRecord,
  type MessageRecord,
  type SendMode,
} from "@reticulum/node-client";

import { useNodeStore } from "./nodeStore";

export const MESSAGE_STORAGE_KEY = "reticulum.mobile.inbox.v1";
export const DIRECT_CHAT_CONNECT_TIMEOUT_MS = 7_000;
export const DIRECT_CHAT_CONNECT_POLL_MS = 250;

export type StoredMessages = Record<string, MessageRecord>;
export type ConversationListItem = {
  conversationId: string;
  destinationHex: string;
  displayName: string;
  preview: string;
  updatedAtMs: number;
  state: string;
};

export function cloneMessage(message: MessageRecord): MessageRecord {
  return {
    ...message,
    bodyUtf8: typeof message.bodyUtf8 === "string" ? message.bodyUtf8 : "",
    title: typeof message.title === "string" ? message.title : undefined,
    detail: typeof message.detail === "string" ? message.detail : undefined,
  };
}

export function safeMessageBody(message: Pick<MessageRecord, "bodyUtf8">): string {
  return typeof message.bodyUtf8 === "string" ? message.bodyUtf8.trim() : "";
}

export function chatNotificationKey(message: MessageRecord): string {
  return message.messageIdHex.trim().toLowerCase();
}

export function loadWebMessages(): StoredMessages {
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

export function saveWebMessages(messages: StoredMessages): void {
  localStorage.setItem(MESSAGE_STORAGE_KEY, JSON.stringify(Object.values(messages)));
}

export function normalizeDestinationHex(value: string): string {
  return value.trim().toLowerCase();
}

export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

export function isDestinationHash(value: string | undefined): boolean {
  return typeof value === "string" && /^[0-9a-f]{32}$/i.test(value.trim());
}

export function peerForDestination(
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

export function peerDisplayName(
  peer: ReturnType<typeof peerForDestination>,
): string | undefined {
  return peer?.announcedName?.trim() || peer?.label?.trim() || undefined;
}

export function displayNameForDestination(
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

export function activePeerForDestination(
  destinationHex: string,
  nodeStore: ReturnType<typeof useNodeStore>,
): boolean {
  return peerForDestination(destinationHex, nodeStore)?.activeLink === true;
}

export function savedPeerRouteForDestination(
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

export function hasKnownLxmfRoute(
  destinationHex: string,
  nodeStore: ReturnType<typeof useNodeStore>,
): boolean {
  const peer = peerForDestination(destinationHex, nodeStore);
  const saved = savedPeerRouteForDestination(destinationHex, nodeStore);
  return isDestinationHash(peer?.lxmfDestinationHex)
    || isDestinationHash(saved?.lxmfDestinationHex);
}

export function canUseRelayChat(
  destinationHex: string,
  nodeStore: ReturnType<typeof useNodeStore>,
): boolean {
  return Boolean(nodeStore.bestPropagationNodeHex)
    && Boolean(savedPeerRouteForDestination(destinationHex, nodeStore))
    && hasKnownLxmfRoute(destinationHex, nodeStore);
}

export function chatSendModeForDestination(
  destinationHex: string,
  nodeStore: ReturnType<typeof useNodeStore>,
): SendMode {
  return canUseRelayChat(destinationHex, nodeStore) ? "Auto" : "DirectOnly";
}

export function peerConnectionDestination(
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

export async function ensureDirectChatPeerConnected(
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

export function remoteDestinationForMessage(message: MessageRecord): string {
  if (message.direction === "Inbound") {
    return normalizeDestinationHex(message.sourceHex ?? "") || normalizeDestinationHex(message.destinationHex);
  }
  return normalizeDestinationHex(message.destinationHex);
}

export function knownConversationDestinations(
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

export function conversationAliasKey(
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

export function collapseConversationItems(
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

export function conversationMatchesDestination(
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

export function draftConversationId(destinationHex: string): string {
  return `draft:${normalizeDestinationHex(destinationHex)}`;
}

export function isDraftConversationId(value: string): boolean {
  return value.startsWith("draft:");
}

export function canonicalConversationIdForDraft(conversationId: string): string {
  if (!isDraftConversationId(conversationId)) {
    return conversationId.trim();
  }
  return normalizeDestinationHex(conversationId.slice("draft:".length));
}

export function messageTimestamp(message: MessageRecord): number {
  return message.receivedAtMs ?? message.sentAtMs ?? message.updatedAtMs;
}

export function mapConversationRecord(
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
