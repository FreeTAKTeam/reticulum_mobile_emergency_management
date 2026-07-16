import type { AnnounceClass, AnnounceDestinationKind, AnnounceReceivedEvent, AnnounceRecord, InstalledPluginRecord, InterfaceStatusChangedEvent, InterfaceStatusRecord, NodeStatus, PeerChangedEvent, PeerState, PluginCapabilityRecord, PluginSensorRecord, RuntimeInterfaceReadinessRecord, RuntimeReadinessSnapshot, RuntimeReadinessState, SendOutcome, StatusChangedEvent } from "./contracts";
import { toOptionalNumber } from "./converters";

export function normalizeHex(value: unknown): string {
  return typeof value === "string" ? value.trim().toLowerCase() : "";
}

export function pluginRecord(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
}

export function toPluginCapabilities(value: unknown): PluginCapabilityRecord {
  const raw = pluginRecord(value);
  return {
    eventsPublish: Boolean(raw.eventsPublish ?? raw.events_publish),
    sensorsPublish: Boolean(raw.sensorsPublish ?? raw.sensors_publish),
    lxmfSend: Boolean(raw.lxmfSend ?? raw.lxmf_send),
    lxmfReceive: Boolean(raw.lxmfReceive ?? raw.lxmf_receive),
    notificationsRaise: Boolean(raw.notificationsRaise ?? raw.notifications_raise),
    operationalRead: Boolean(raw.operationalRead ?? raw.operational_read),
  };
}

export function toInstalledPlugin(raw: Record<string, unknown>): InstalledPluginRecord {
  const state = String(raw.state ?? "Discovered") as InstalledPluginRecord["state"];
  const messages = Array.isArray(raw.messages)
    ? raw.messages.map((entry) => {
        const message = pluginRecord(entry);
        return {
          name: String(message.name ?? ""),
          version: String(message.version ?? ""),
          send: Boolean(message.send),
          receive: Boolean(message.receive),
          schema: pluginRecord(message.schema),
        };
      })
    : [];
  return {
    pluginId: String(raw.pluginId ?? raw.plugin_id ?? ""),
    displayName: String(raw.displayName ?? raw.display_name ?? ""),
    version: String(raw.version ?? ""),
    apiMajor: Number(raw.apiMajor ?? raw.api_major ?? 0),
    apiMinor: Number(raw.apiMinor ?? raw.api_minor ?? 0),
    packageName: String(raw.packageName ?? raw.package_name ?? ""),
    serviceClassName: String(raw.serviceClassName ?? raw.service_class_name ?? ""),
    publisherFingerprint: normalizeHex(raw.publisherFingerprint ?? raw.publisher_fingerprint),
    publisherHistory: Array.isArray(raw.publisherHistory)
      ? raw.publisherHistory.map((value) => normalizeHex(value))
      : [],
    androidPermissions: Array.isArray(raw.androidPermissions)
      ? raw.androidPermissions.map(String)
      : [],
    declaredCapabilities: toPluginCapabilities(
      raw.declaredCapabilities ?? raw.declared_capabilities,
    ),
    messages,
    configurationEntrypoint:
      typeof (raw.configurationEntrypoint ?? raw.configuration_entrypoint) === "string"
        ? String(raw.configurationEntrypoint ?? raw.configuration_entrypoint)
        : undefined,
    state,
    trusted: Boolean(raw.trusted),
    enabled: Boolean(raw.enabled),
    grantedCapabilities: toPluginCapabilities(
      raw.grantedCapabilities ?? raw.granted_capabilities,
    ),
    diagnostic: typeof raw.diagnostic === "string" ? raw.diagnostic : undefined,
    updatedAtMs: Number(raw.updatedAtMs ?? raw.updated_at_ms ?? 0),
  };
}

export function toPluginSensor(raw: Record<string, unknown>): PluginSensorRecord {
  const status = String(raw.status ?? "Offline") as PluginSensorRecord["status"];
  const origin = String(raw.origin ?? "local") as PluginSensorRecord["origin"];
  return {
    pluginId: String(raw.pluginId ?? raw.plugin_id ?? ""),
    deviceId: String(raw.deviceId ?? raw.device_id ?? ""),
    sensorType: String(raw.sensorType ?? raw.sensor_type ?? ""),
    displayName: String(raw.displayName ?? raw.display_name ?? ""),
    value: raw.value,
    unit: typeof raw.unit === "string" ? raw.unit : undefined,
    operatorRnsIdentity:
      typeof (raw.operatorRnsIdentity ?? raw.operator_rns_identity) === "string"
        ? String(raw.operatorRnsIdentity ?? raw.operator_rns_identity)
        : undefined,
    confidence: toOptionalNumber(raw.confidence),
    connectionState:
      typeof (raw.connectionState ?? raw.connection_state) === "string"
        ? String(raw.connectionState ?? raw.connection_state)
        : undefined,
    sampleAtMs: Number(raw.sampleAtMs ?? raw.sample_at_ms ?? 0),
    staleAfterMs: Number(raw.staleAfterMs ?? raw.stale_after_ms ?? 0),
    status,
    origin,
  };
}

export function hasValue(value: unknown): boolean {
  return value !== undefined && value !== null;
}

export function toOptionalHex(value: unknown): string | undefined {
  if (!hasValue(value)) {
    return undefined;
  }
  const normalized = normalizeHex(value);
  return normalized ? normalized : undefined;
}

export function toOptionalBoolean(value: unknown): boolean | undefined {
  if (!hasValue(value)) {
    return undefined;
  }
  return Boolean(value);
}

export function decodeBase64ToBytes(value: string): Uint8Array {
  const bufferCtor = (
    globalThis as unknown as {
      Buffer?: { from(data: string, encoding: string): Uint8Array };
    }
  ).Buffer;
  if (bufferCtor) {
    return Uint8Array.from(bufferCtor.from(value, "base64"));
  }
  const binary = atob(value);
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    out[i] = binary.charCodeAt(i);
  }
  return out;
}

export function encodeBytesToBase64(value: Uint8Array): string {
  const bufferCtor = (
    globalThis as unknown as {
      Buffer?: { from(data: Uint8Array): { toString(encoding: string): string } };
    }
  ).Buffer;
  if (bufferCtor) {
    return bufferCtor.from(value).toString("base64");
  }
  let binary = "";
  for (const v of value) {
    binary += String.fromCharCode(v);
  }
  return btoa(binary);
}

export function toNodeStatus(raw: Record<string, unknown>): NodeStatus {
  const interfacesRaw = raw.interfaces ?? raw.interface_statuses;
  return {
    running: Boolean(raw.running),
    name: String(raw.name ?? ""),
    identityHex: String(raw.identityHex ?? raw.identity_hex ?? ""),
    appDestinationHex: String(
      raw.appDestinationHex ?? raw.app_destination_hex ?? "",
    ),
    lxmfDestinationHex: String(
      raw.lxmfDestinationHex ?? raw.lxmf_destination_hex ?? "",
    ),
    lastError:
      typeof raw.lastError === "string"
        ? raw.lastError
        : typeof raw.last_error === "string"
          ? raw.last_error
          : undefined,
    readiness: toRuntimeReadinessSnapshot(raw.readiness),
    interfaces: Array.isArray(interfacesRaw)
      ? interfacesRaw.map((entry) => toInterfaceStatusRecord(entry)).filter((entry) => entry.interfaceHex.length > 0)
      : [],
  };
}

export function toRuntimeReadinessState(raw: unknown): RuntimeReadinessState {
  switch (enumVariantName(raw)) {
    case "Ready":
      return "Ready";
    case "Failed":
      return "Failed";
    case "Unsupported":
      return "Unsupported";
    case "Disabled":
      return "Disabled";
    case "Pending":
    default:
      return "Pending";
  }
}

export function toRuntimeReadinessSnapshot(raw: unknown): RuntimeReadinessSnapshot {
  const record = raw && typeof raw === "object" && !Array.isArray(raw)
    ? raw as Record<string, unknown>
    : {};
  const interfaces = Array.isArray(record.interfaces)
    ? record.interfaces.map((entry): RuntimeInterfaceReadinessRecord => {
        const item = entry && typeof entry === "object" && !Array.isArray(entry)
          ? entry as Record<string, unknown>
          : {};
        return {
          id: String(item.id ?? ""),
          label: String(item.label ?? ""),
          state: toRuntimeReadinessState(item.state),
          detail: String(item.detail ?? ""),
          lastError: typeof item.lastError === "string"
            ? item.lastError
            : typeof item.last_error === "string"
              ? item.last_error
              : undefined,
        };
      })
    : [];
  return {
    state: Object.keys(record).length > 0
      ? toRuntimeReadinessState(record.state)
      : "Pending",
    interfaces,
  };
}

export function toInterfaceStatusRecord(raw: unknown): InterfaceStatusRecord {
  const record = raw && typeof raw === "object" && !Array.isArray(raw)
    ? raw as Record<string, unknown>
    : {};
  return {
    interfaceHex: String(record.interfaceHex ?? record.interface_hex ?? ""),
    label: String(record.label ?? ""),
    kind: String(record.kind ?? ""),
    state: String(record.state ?? ""),
    lastError:
      typeof record.lastError === "string"
        ? record.lastError
        : typeof record.last_error === "string"
          ? record.last_error
          : undefined,
    rxPackets: Number(record.rxPackets ?? record.rx_packets ?? 0),
    rxBytes: Number(record.rxBytes ?? record.rx_bytes ?? 0),
    lastActivityMs: Number(record.lastActivityMs ?? record.last_activity_ms ?? 0),
  };
}

export function enumVariantName(raw: unknown): string {
  if (typeof raw === "string") {
    return raw.trim();
  }
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
    return "";
  }
  const variants = Object.keys(raw as Record<string, unknown>).filter((key) => key.trim().length > 0);
  return variants.length === 1 ? variants[0]!.trim() : "";
}

export function toPeerState(raw: unknown): PeerState {
  const value = enumVariantName(raw);
  switch (value.toLowerCase()) {
    case "connecting":
      return "Connecting";
    case "connected":
      return "Connected";
    case "disconnected":
      return "Disconnected";
    default:
      return "Disconnected";
  }
}

export function toSavedFlag(raw: unknown, legacyManagementRaw?: unknown): boolean {
  if (typeof raw === "boolean") {
    return raw;
  }
  if (hasValue(raw)) {
    return Boolean(raw);
  }
  return enumVariantName(legacyManagementRaw).toLowerCase() === "managed";
}

export function toSendOutcome(raw: unknown): SendOutcome {
  const value = String(raw ?? "");
  const valid: SendOutcome[] = [
    "SentDirect",
    "SentBroadcast",
    "DroppedMissingDestinationIdentity",
    "DroppedCiphertextTooLarge",
    "DroppedEncryptFailed",
    "DroppedNoRoute",
  ];
  return valid.includes(value as SendOutcome)
    ? (value as SendOutcome)
    : "DroppedNoRoute";
}

export function toStatusChangedEvent(raw: Record<string, unknown>): StatusChangedEvent {
  const statusRaw =
    (raw.status as Record<string, unknown> | undefined) ?? raw;
  return { status: toNodeStatus(statusRaw) };
}

export function toInterfaceStatusChangedEvent(raw: Record<string, unknown>): InterfaceStatusChangedEvent {
  const statusRaw = raw.status ?? raw;
  return { status: toInterfaceStatusRecord(statusRaw) };
}

export function toAnnounceReceivedEvent(
  raw: Record<string, unknown>,
): AnnounceReceivedEvent {
  const destinationKindRaw = String(
    raw.destinationKind ?? raw.destination_kind ?? "other",
  );
  const destinationKind: AnnounceDestinationKind =
    destinationKindRaw === "app"
      || destinationKindRaw === "lxmf_delivery"
      || destinationKindRaw === "lxmf_propagation"
      ? destinationKindRaw
      : "other";
  const announceClassRaw = String(
    raw.announceClass ?? raw.announce_class ?? "Other",
  );
  const announceClass: AnnounceClass =
    announceClassRaw === "PeerApp"
      || announceClassRaw === "RchHubServer"
      || announceClassRaw === "PropagationNode"
      || announceClassRaw === "LxmfDelivery"
      ? announceClassRaw
      : "Other";
  return {
    destinationHex: normalizeHex(
      String(raw.destinationHex ?? raw.destination_hex ?? ""),
    ),
    identityHex: normalizeHex(
      String(raw.identityHex ?? raw.identity_hex ?? ""),
    ),
    destinationKind,
    announceClass,
    appData: String(raw.appData ?? raw.app_data ?? ""),
    displayName:
      typeof raw.displayName === "string"
        ? raw.displayName
        : typeof raw.display_name === "string"
          ? raw.display_name
          : undefined,
    hops: Number(raw.hops ?? 0),
    interfaceHex: String(raw.interfaceHex ?? raw.interface_hex ?? ""),
    receivedAtMs: Number(raw.receivedAtMs ?? raw.received_at_ms ?? Date.now()),
  };
}

export function toAnnounceRecord(raw: Record<string, unknown>): AnnounceRecord {
  const event = toAnnounceReceivedEvent(raw);
  return {
    ...event,
    displayName:
      typeof raw.displayName === "string"
        ? raw.displayName
        : typeof raw.display_name === "string"
          ? raw.display_name
          : undefined,
  };
}

export function toPeerChangedEvent(raw: Record<string, unknown>): PeerChangedEvent {
  const changeRaw = (raw.change as Record<string, unknown> | undefined) ?? raw;
  const activeLinkRaw = hasValue(changeRaw.activeLink)
    ? changeRaw.activeLink
    : changeRaw.active_link;
  const lastSeenAtMsRaw = hasValue(changeRaw.lastSeenAtMs)
    ? changeRaw.lastSeenAtMs
    : changeRaw.last_seen_at_ms;
  return {
    change: {
      destinationHex: normalizeHex(
        String(changeRaw.destinationHex ?? changeRaw.destination_hex ?? ""),
      ),
      identityHex: toOptionalHex(
        hasValue(changeRaw.identityHex) ? changeRaw.identityHex : changeRaw.identity_hex,
      ),
      lxmfDestinationHex: toOptionalHex(
        hasValue(changeRaw.lxmfDestinationHex)
          ? changeRaw.lxmfDestinationHex
          : changeRaw.lxmf_destination_hex,
      ),
      displayName:
        typeof changeRaw.displayName === "string"
          ? changeRaw.displayName
          : typeof changeRaw.display_name === "string"
            ? changeRaw.display_name
            : undefined,
      appData:
        typeof changeRaw.appData === "string"
          ? changeRaw.appData
          : typeof changeRaw.app_data === "string"
            ? changeRaw.app_data
            : undefined,
      state: hasValue(changeRaw.state) ? toPeerState(changeRaw.state) : undefined,
      saved: toSavedFlag(changeRaw.saved, changeRaw.managementState ?? changeRaw.management_state),
      stale: Boolean(changeRaw.stale),
      activeLink: Boolean(activeLinkRaw),
      hubDerived: Boolean(
        hasValue(changeRaw.hubDerived) ? changeRaw.hubDerived : changeRaw.hub_derived,
      ),
      lastError: (changeRaw.lastError ?? changeRaw.last_error) as
        | string
        | undefined,
      lastResolutionError:
        typeof changeRaw.lastResolutionError === "string"
          ? changeRaw.lastResolutionError
          : typeof changeRaw.last_resolution_error === "string"
            ? changeRaw.last_resolution_error
            : undefined,
      lastResolutionAttemptAtMs: toOptionalNumber(
        hasValue(changeRaw.lastResolutionAttemptAtMs)
          ? changeRaw.lastResolutionAttemptAtMs
          : changeRaw.last_resolution_attempt_at_ms,
      ),
      lastSeenAtMs: toOptionalNumber(lastSeenAtMsRaw),
      announceLastSeenAtMs: toOptionalNumber(
        hasValue(changeRaw.announceLastSeenAtMs)
          ? changeRaw.announceLastSeenAtMs
          : changeRaw.announce_last_seen_at_ms,
      ),
      lxmfLastSeenAtMs: toOptionalNumber(
        hasValue(changeRaw.lxmfLastSeenAtMs)
          ? changeRaw.lxmfLastSeenAtMs
          : changeRaw.lxmf_last_seen_at_ms,
      ),
    },
  };
}

