import { defineStore } from "pinia";
import { computed, reactive, ref, watch } from "vue";
import type { PacketReceivedEvent } from "@reticulum/node-client";

import type { ActionMessage, EamStatus, ReplicationMessage } from "../types/domain";
import { useNodeStore } from "./nodeStore";

const MESSAGE_STORAGE_KEY = "reticulum.mobile.messages.v1";

const STATUS_ROTATION: EamStatus[] = ["Green", "Yellow", "Red"];

function loadMessages(): Record<string, ActionMessage> {
  try {
    const raw = localStorage.getItem(MESSAGE_STORAGE_KEY);
    if (!raw) {
      return {};
    }
    const parsed = JSON.parse(raw) as ActionMessage[];
    const out: Record<string, ActionMessage> = {};
    for (const message of parsed) {
      const callsign = String(message.callsign ?? "").trim();
      if (!callsign) {
        continue;
      }
      out[callsign.toLowerCase()] = {
        ...message,
        callsign,
        groupName: String(message.groupName ?? "Cal team"),
        updatedAt: Number(message.updatedAt ?? Date.now()),
      };
    }
    return out;
  } catch {
    return {};
  }
}

function saveMessages(messages: Record<string, ActionMessage>): void {
  localStorage.setItem(MESSAGE_STORAGE_KEY, JSON.stringify(Object.values(messages)));
}

function cloneMessage(message: ActionMessage): ActionMessage {
  return {
    ...message,
  };
}

function normalizeStatus(value: EamStatus): EamStatus {
  if (value === "Green" || value === "Yellow" || value === "Red") {
    return value;
  }
  return "Unknown";
}

function normalizeMessage(message: ActionMessage): ActionMessage {
  return {
    ...message,
    callsign: message.callsign.trim(),
    groupName: message.groupName.trim(),
    securityStatus: normalizeStatus(message.securityStatus),
    capabilityStatus: normalizeStatus(message.capabilityStatus),
    preparednessStatus: normalizeStatus(message.preparednessStatus),
    medicalStatus: normalizeStatus(message.medicalStatus),
    mobilityStatus: normalizeStatus(message.mobilityStatus),
    commsStatus: normalizeStatus(message.commsStatus),
    updatedAt: Number(message.updatedAt || Date.now()),
  };
}

export const useMessagesStore = defineStore("messages", () => {
  const byCallsign = reactive<Record<string, ActionMessage>>({});
  const initialized = ref(false);
  const replicationInitialized = ref(false);
  const nodeStore = useNodeStore();

  function persist(): void {
    saveMessages(byCallsign);
  }

  function init(): void {
    if (initialized.value) {
      return;
    }
    initialized.value = true;

    const loaded = loadMessages();
    for (const [key, message] of Object.entries(loaded)) {
      byCallsign[key] = cloneMessage(message);
    }
  }

  function keyFor(callsign: string): string {
    return callsign.trim().toLowerCase();
  }

  function applyUpsert(message: ActionMessage): void {
    const normalized = normalizeMessage(message);
    const key = keyFor(normalized.callsign);
    const existing = byCallsign[key];
    if (existing && existing.updatedAt > normalized.updatedAt) {
      return;
    }
    byCallsign[key] = normalized;
    persist();
  }

  function applyDelete(callsign: string, deletedAt: number): void {
    const key = keyFor(callsign);
    const existing = byCallsign[key];
    if (!existing) {
      return;
    }
    if (existing.updatedAt > deletedAt) {
      return;
    }
    byCallsign[key] = {
      ...existing,
      deletedAt,
      updatedAt: deletedAt,
    };
    persist();
  }

  async function upsertLocal(
    next: Omit<ActionMessage, "updatedAt" | "deletedAt"> & {
      updatedAt?: number;
    },
  ): Promise<void> {
    const updatedAt = Number(next.updatedAt ?? Date.now());
    const message: ActionMessage = normalizeMessage({
      ...next,
      updatedAt,
    });
    applyUpsert(message);
    await nodeStore.broadcastJson({
      kind: "message_upsert",
      message,
    } as ReplicationMessage);
  }

  async function deleteLocal(callsign: string): Promise<void> {
    const deletedAt = Date.now();
    applyDelete(callsign, deletedAt);
    await nodeStore.broadcastJson({
      kind: "message_delete",
      callsign,
      deletedAt,
    } as ReplicationMessage);
  }

  function rotateStatus(callsign: string, field: keyof ActionMessage): void {
    const key = keyFor(callsign);
    const current = byCallsign[key];
    if (!current || current.deletedAt) {
      return;
    }
    const value = current[field];
    if (
      field !== "securityStatus" &&
      field !== "capabilityStatus" &&
      field !== "preparednessStatus" &&
      field !== "medicalStatus" &&
      field !== "mobilityStatus" &&
      field !== "commsStatus"
    ) {
      return;
    }
    const idx = STATUS_ROTATION.indexOf(normalizeStatus(value as EamStatus));
    const nextStatus = STATUS_ROTATION[(idx + 1) % STATUS_ROTATION.length];
    upsertLocal({
      ...current,
      [field]: nextStatus,
    }).catch(() => undefined);
  }

  function parseReplicationMessage(raw: string): ReplicationMessage | null {
    try {
      const parsed = JSON.parse(raw) as Partial<ReplicationMessage>;
      if (!parsed || typeof parsed !== "object" || !("kind" in parsed)) {
        return null;
      }
      switch (parsed.kind) {
        case "snapshot_request":
          return {
            kind: "snapshot_request",
            requestedAt: Number(parsed.requestedAt ?? Date.now()),
          };
        case "snapshot_response":
          return {
            kind: "snapshot_response",
            requestedAt: Number(parsed.requestedAt ?? Date.now()),
            messages: Array.isArray(parsed.messages)
              ? parsed.messages.map((entry) => normalizeMessage(entry as ActionMessage))
              : [],
          };
        case "message_upsert":
          if (!parsed.message) {
            return null;
          }
          return {
            kind: "message_upsert",
            message: normalizeMessage(parsed.message as ActionMessage),
          };
        case "message_delete":
          return {
            kind: "message_delete",
            callsign: String(parsed.callsign ?? ""),
            deletedAt: Number(parsed.deletedAt ?? Date.now()),
          };
        default:
          return null;
      }
    } catch {
      return null;
    }
  }

  function snapshotMessages(): ActionMessage[] {
    return Object.values(byCallsign).map((message) => cloneMessage(message));
  }

  function initReplication(): void {
    if (replicationInitialized.value) {
      return;
    }
    replicationInitialized.value = true;

    const decoder = new TextDecoder();
    nodeStore.onPacket((event: PacketReceivedEvent) => {
      const raw = decoder.decode(event.bytes);
      const message = parseReplicationMessage(raw);
      if (!message) {
        return;
      }

      if (message.kind === "snapshot_request") {
        nodeStore
          .broadcastJson({
            kind: "snapshot_response",
            requestedAt: message.requestedAt,
            messages: snapshotMessages(),
          } as ReplicationMessage)
          .catch(() => undefined);
        return;
      }

      if (message.kind === "snapshot_response") {
        for (const incoming of message.messages) {
          if (incoming.deletedAt) {
            applyDelete(incoming.callsign, incoming.deletedAt);
          } else {
            applyUpsert(incoming);
          }
        }
        return;
      }

      if (message.kind === "message_upsert") {
        applyUpsert(message.message);
        return;
      }

      if (message.kind === "message_delete") {
        applyDelete(message.callsign, message.deletedAt);
      }
    });

    watch(
      () => [...nodeStore.connectedDestinations],
      (current, previous) => {
        const previousSet = new Set(previous);
        for (const destination of current) {
          if (previousSet.has(destination)) {
            continue;
          }
          nodeStore
            .sendJson(destination, {
              kind: "snapshot_request",
              requestedAt: Date.now(),
            } as ReplicationMessage)
            .catch(() => undefined);
        }
      },
      { immediate: true },
    );
  }

  const messages = computed(() =>
    Object.values(byCallsign)
      .filter((message) => !message.deletedAt)
      .sort((a, b) => b.updatedAt - a.updatedAt),
  );

  const activeCount = computed(() => messages.value.length);

  const redCount = computed(
    () =>
      messages.value.filter(
        (message) =>
          message.securityStatus === "Red" ||
          message.mobilityStatus === "Red" ||
          message.medicalStatus === "Red",
      ).length,
  );

  return {
    byCallsign,
    messages,
    activeCount,
    redCount,
    init,
    initReplication,
    upsertLocal,
    deleteLocal,
    rotateStatus,
  };
});
