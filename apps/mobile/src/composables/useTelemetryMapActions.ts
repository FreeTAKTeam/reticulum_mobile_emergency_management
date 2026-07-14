import { useRouter } from "vue-router";

import { useMessagesStore } from "../stores/messagesStore";
import { useMessagingStore } from "../stores/messagingStore";
import { useNodeStore } from "../stores/nodeStore";
import type { TelemetryPosition } from "../types/domain";
import {
  buildTelemetryPopupHtml,
  chatDestinationForPeer,
  createTelemetryPopupElement,
  eamMessageForPosition,
  eamPieHtml,
  peerDisplayName,
  peerForPosition,
  positionLabel as resolvePositionLabel,
  safeTrim,
} from "../utils/telemetryMapModel";

export function useTelemetryMapActions() {
  const messagesStore = useMessagesStore();
  const messagingStore = useMessagingStore();
  const nodeStore = useNodeStore();
  const router = useRouter();

  function positionLabel(position: TelemetryPosition): string {
    return resolvePositionLabel(position, nodeStore.discoveredByDestination);
  }

  function openEamDetails(callsign: string): void {
    const targetCallsign = safeTrim(callsign);
    void router.push({
      name: "messages",
      query: targetCallsign ? { callsign: targetCallsign } : undefined,
    });
  }

  async function openChat(position: TelemetryPosition): Promise<void> {
    const label = positionLabel(position);
    const peer = peerForPosition(position, label, nodeStore.discoveredByDestination);
    const destinationHex = chatDestinationForPeer(peer);
    if (!destinationHex) return;
    messagingStore.ensureConversationForDestination(
      destinationHex,
      peerDisplayName(peer, label),
    );
    await router.push({
      path: "/inbox",
      query: messagingStore.selectedConversationId
        ? { conversation: messagingStore.selectedConversationId }
        : undefined,
    });
  }

  function popupElement(position: TelemetryPosition): HTMLDivElement {
    const label = positionLabel(position);
    const message = eamMessageForPosition(position, label, messagesStore.messages);
    const readiness = message
      ? messagesStore.eamReadinessForCallsign(message.callsign)
      : undefined;
    return createTelemetryPopupElement(
      buildTelemetryPopupHtml(position, label, eamPieHtml(message, readiness)),
      () => void openChat(position),
      () => openEamDetails(position.callsign),
    );
  }

  return { popupElement, positionLabel };
}
