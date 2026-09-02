import {
  type ConversationRecord,
  type MessageRecord,
} from "@reticulum/node-client";
import type { Ref } from "vue";

import {
  type ConversationListItem,
  type StoredMessages,
  cloneMessage,
  normalizeDestinationHex,
} from "./messagingModel";
import { useNodeStore } from "./nodeStore";

interface MessagingVisualMockContext {
  byMessageId: Ref<StoredMessages>;
  getConversations: () => ConversationListItem[];
  hydrated: Ref<boolean>;
  nativeConversations: Ref<ConversationRecord[]>;
  nodeStore: ReturnType<typeof useNodeStore>;
  pendingConversation: Ref<ConversationListItem | null>;
  selectedConversationId: Ref<string>;
  selectedTargetMessageId: Ref<string>;
  upsertMessage: (message: MessageRecord) => void;
  visualMockDeletedConversationIds: Ref<Set<string>>;
}

export function createMessagingVisualMock(context: MessagingVisualMockContext) {
  const {
    byMessageId,
    getConversations,
    hydrated,
    nativeConversations,
    nodeStore,
    pendingConversation,
    selectedConversationId,
    selectedTargetMessageId,
    upsertMessage,
    visualMockDeletedConversationIds,
  } = context;

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
        circleTier: "inner",
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
        requestedDestinationHex: peerRecords[0].lxmfDestinationHex,
        deliveryDestinationHex: peerRecords[0].lxmfDestinationHex,
        lastWireMessageIdHex: "10000000000000000000000000000001",
        title: "Status check",
        bodyUtf8: "Alpha team is staged at checkpoint A. Radio relay is stable.",
        trafficClass: "chat",
        method: "Direct",
        state: "Received",
        transportState: "TransportDelivered",
        applicationAckState: "NotRequired",
        receivedAtMs: now - 9 * 60_000,
        updatedAtMs: now - 9 * 60_000,
      },
      {
        messageIdHex: "10000000000000000000000000000002",
        conversationId: peerRecords[0].lxmfDestinationHex,
        direction: "Outbound",
        destinationHex: peerRecords[0].lxmfDestinationHex,
        sourceHex: localLxmfDestination,
        requestedDestinationHex: peerRecords[0].lxmfDestinationHex,
        deliveryDestinationHex: peerRecords[0].lxmfDestinationHex,
        lastWireMessageIdHex: "10000000000000000000000000000002",
        bodyUtf8: "Send two operators to the north entrance and confirm when they are in position.",
        trafficClass: "chat",
        method: "Direct",
        state: "Delivered",
        transportState: "TransportDelivered",
        applicationAckState: "Accepted",
        sentAtMs: now - 3 * 60_000,
        updatedAtMs: now - 2 * 60_000,
      },
      {
        messageIdHex: "10000000000000000000000000000003",
        conversationId: peerRecords[0].lxmfDestinationHex,
        direction: "Inbound",
        destinationHex: peerRecords[0].lxmfDestinationHex,
        sourceHex: peerRecords[0].lxmfDestinationHex,
        requestedDestinationHex: peerRecords[0].lxmfDestinationHex,
        deliveryDestinationHex: peerRecords[0].lxmfDestinationHex,
        lastWireMessageIdHex: "10000000000000000000000000000003",
        bodyUtf8: "Copy. Two operators moving to the north entrance now.",
        trafficClass: "chat",
        method: "Direct",
        state: "Received",
        transportState: "TransportDelivered",
        applicationAckState: "NotRequired",
        receivedAtMs: now - 45_000,
        updatedAtMs: now - 45_000,
      },
      {
        messageIdHex: "20000000000000000000000000000001",
        conversationId: peerRecords[2].lxmfDestinationHex,
        direction: "Inbound",
        destinationHex: peerRecords[2].lxmfDestinationHex,
        sourceHex: peerRecords[2].lxmfDestinationHex,
        requestedDestinationHex: peerRecords[2].lxmfDestinationHex,
        deliveryDestinationHex: peerRecords[2].lxmfDestinationHex,
        lastWireMessageIdHex: "20000000000000000000000000000001",
        title: "Medical",
        bodyUtf8: "Emergency: patient transport requested at triage tent.\nGPS: 46.81,-71.20",
        trafficClass: "chat",
        method: "Direct",
        state: "Received",
        transportState: "TransportDelivered",
        applicationAckState: "NotRequired",
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
        requestedDestinationHex: peerRecords[2].lxmfDestinationHex,
        deliveryDestinationHex: peerRecords[2].lxmfDestinationHex,
        lastWireMessageIdHex: "20000000000000000000000000000002",
        bodyUtf8: "Transport team notified. Keep the patient at the marked triage point.",
        trafficClass: "chat",
        method: "Direct",
        state: "SentDirect",
        transportState: "SentDirect",
        applicationAckState: "Waiting",
        sentAtMs: now - 2 * 60_000,
        updatedAtMs: now - 2 * 60_000,
      },
      {
        messageIdHex: "30000000000000000000000000000001",
        conversationId: peerRecords[1].lxmfDestinationHex,
        direction: "Inbound",
        destinationHex: peerRecords[1].lxmfDestinationHex,
        sourceHex: peerRecords[1].lxmfDestinationHex,
        requestedDestinationHex: peerRecords[1].lxmfDestinationHex,
        deliveryDestinationHex: peerRecords[1].lxmfDestinationHex,
        lastWireMessageIdHex: "30000000000000000000000000000001",
        bodyUtf8: "North checkpoint is intermittent. Propagation relay is preferred until the link is stable.",
        trafficClass: "chat",
        method: "Propagated",
        state: "Received",
        transportState: "TransportDelivered",
        applicationAckState: "NotRequired",
        receivedAtMs: now - 21 * 60_000,
        updatedAtMs: now - 21 * 60_000,
      },
      {
        messageIdHex: "30000000000000000000000000000002",
        conversationId: peerRecords[1].lxmfDestinationHex,
        direction: "Outbound",
        destinationHex: peerRecords[1].lxmfDestinationHex,
        sourceHex: localLxmfDestination,
        requestedDestinationHex: peerRecords[1].lxmfDestinationHex,
        deliveryDestinationHex: peerRecords[1].lxmfDestinationHex,
        lastWireMessageIdHex: "30000000000000000000000000000002",
        title: "Route update",
        bodyUtf8: "Route update queued through propagation. Confirm when NORTH-CP comes back online.",
        trafficClass: "chat",
        method: "Propagated",
        state: "SentToPropagation",
        transportState: "SentToPropagation",
        applicationAckState: "Waiting",
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
    const conversation = getConversations().find((item) =>
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
      requestedDestinationHex: normalizedDestination,
      deliveryDestinationHex: conversation?.destinationHex ?? normalizedDestination,
      lastWireMessageIdHex: `900000000000000000000000${now.toString(16).slice(-8)}`,
      bodyUtf8,
      trafficClass: "chat",
      method: "Opportunistic",
      state: "Queued",
      transportState: "Queued",
      applicationAckState: "Waiting",
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

  return {
    appendVisualMockOutboundMessage,
    applyVisualMockChatData,
    isVisualMockConversationDeleted,
    markVisualMockConversationDeleted,
    messageBelongsToDeletedVisualMockConversation,
  };
}
