import type { NodeConfig, OperationalSummary, SosAlertRecord, SosAudioRecord, SosLocationRecord, SosMessageKind, SosSettingsRecord, SosState, SosStatusRecord, SosTriggerSource } from "./contracts";
import { toOptionalNumber } from "./converters";
import { decodeBase64ToBytes, encodeBytesToBase64, enumVariantName, normalizeHex, pluginRecord, toOptionalHex } from "./runtime-converters";

export const DEFAULT_SOS_SETTINGS: SosSettingsRecord = {
  enabled: false,
  messageTemplate: "SOS! I need help...",
  cancelMessageTemplate: "SOS Cancelled - I am safe.",
  countdownSeconds: 5,
  includeLocation: true,
  triggerShake: false,
  triggerTapPattern: false,
  triggerPowerButton: false,
  shakeSensitivity: 2.5,
  audioRecording: false,
  audioDurationSeconds: 30,
  periodicUpdates: false,
  updateIntervalSeconds: 120,
  floatingButton: false,
  silentAutoAnswer: false,
  floatingButtonX: 24,
  floatingButtonY: 440,
  activePillX: 24,
  activePillY: 24,
};

export const DEFAULT_SOS_STATUS: SosStatusRecord = {
  state: "Idle",
  updatedAtMs: 0,
};

export function toSosState(value: unknown): SosState {
  const normalized = String(value ?? "Idle");
  return normalized === "Countdown" || normalized === "Sending" || normalized === "Active"
    ? normalized
    : "Idle";
}

export function toSosTriggerSource(value: unknown): SosTriggerSource | undefined {
  const normalized = String(value ?? "");
  if (
    normalized === "Manual"
    || normalized === "FloatingButton"
    || normalized === "Shake"
    || normalized === "TapPattern"
    || normalized === "PowerButton"
    || normalized === "Restore"
    || normalized === "Remote"
  ) {
    return normalized;
  }
  return undefined;
}

export function toSosMessageKind(value: unknown): SosMessageKind {
  const normalized = String(value ?? "Active");
  return normalized === "Update" || normalized === "Cancelled" ? normalized : "Active";
}

export function toSosSettingsRecord(raw: Record<string, unknown>): SosSettingsRecord {
  return {
    ...DEFAULT_SOS_SETTINGS,
    enabled: Boolean(raw.enabled),
    messageTemplate: String(raw.messageTemplate ?? raw.message_template ?? DEFAULT_SOS_SETTINGS.messageTemplate),
    cancelMessageTemplate: String(raw.cancelMessageTemplate ?? raw.cancel_message_template ?? DEFAULT_SOS_SETTINGS.cancelMessageTemplate),
    countdownSeconds: Number(raw.countdownSeconds ?? raw.countdown_seconds ?? DEFAULT_SOS_SETTINGS.countdownSeconds),
    includeLocation: Boolean(raw.includeLocation ?? raw.include_location ?? DEFAULT_SOS_SETTINGS.includeLocation),
    triggerShake: Boolean(raw.triggerShake ?? raw.trigger_shake),
    triggerTapPattern: Boolean(raw.triggerTapPattern ?? raw.trigger_tap_pattern),
    triggerPowerButton: Boolean(raw.triggerPowerButton ?? raw.trigger_power_button),
    shakeSensitivity: Number(raw.shakeSensitivity ?? raw.shake_sensitivity ?? DEFAULT_SOS_SETTINGS.shakeSensitivity),
    audioRecording: Boolean(raw.audioRecording ?? raw.audio_recording),
    audioDurationSeconds: Number(raw.audioDurationSeconds ?? raw.audio_duration_seconds ?? DEFAULT_SOS_SETTINGS.audioDurationSeconds),
    periodicUpdates: Boolean(raw.periodicUpdates ?? raw.periodic_updates),
    updateIntervalSeconds: Number(raw.updateIntervalSeconds ?? raw.update_interval_seconds ?? DEFAULT_SOS_SETTINGS.updateIntervalSeconds),
    floatingButton: Boolean(raw.floatingButton ?? raw.floating_button),
    silentAutoAnswer: Boolean(raw.silentAutoAnswer ?? raw.silent_auto_answer),
    deactivationPinHash: typeof raw.deactivationPinHash === "string" ? raw.deactivationPinHash : typeof raw.deactivation_pin_hash === "string" ? raw.deactivation_pin_hash : undefined,
    deactivationPinSalt: typeof raw.deactivationPinSalt === "string" ? raw.deactivationPinSalt : typeof raw.deactivation_pin_salt === "string" ? raw.deactivation_pin_salt : undefined,
    floatingButtonX: Number(raw.floatingButtonX ?? raw.floating_button_x ?? DEFAULT_SOS_SETTINGS.floatingButtonX),
    floatingButtonY: Number(raw.floatingButtonY ?? raw.floating_button_y ?? DEFAULT_SOS_SETTINGS.floatingButtonY),
    activePillX: Number(raw.activePillX ?? raw.active_pill_x ?? DEFAULT_SOS_SETTINGS.activePillX),
    activePillY: Number(raw.activePillY ?? raw.active_pill_y ?? DEFAULT_SOS_SETTINGS.activePillY),
  };
}

export function toSosStatusRecord(raw: Record<string, unknown>): SosStatusRecord {
  const nested = raw.status;
  if (nested && typeof nested === "object" && !Array.isArray(nested)) {
    return toSosStatusRecord(nested as Record<string, unknown>);
  }
  return {
    state: toSosState(raw.state),
    incidentId: typeof raw.incidentId === "string" ? raw.incidentId : typeof raw.incident_id === "string" ? raw.incident_id : undefined,
    triggerSource: toSosTriggerSource(raw.triggerSource ?? raw.trigger_source),
    countdownDeadlineMs: toOptionalNumber(raw.countdownDeadlineMs ?? raw.countdown_deadline_ms),
    activatedAtMs: toOptionalNumber(raw.activatedAtMs ?? raw.activated_at_ms),
    lastSentAtMs: toOptionalNumber(raw.lastSentAtMs ?? raw.last_sent_at_ms),
    lastUpdateAtMs: toOptionalNumber(raw.lastUpdateAtMs ?? raw.last_update_at_ms),
    updatedAtMs: Number(raw.updatedAtMs ?? raw.updated_at_ms ?? Date.now()),
  };
}

export function toSosAlertRecord(raw: Record<string, unknown>): SosAlertRecord {
  const nested = raw.alert;
  if (nested && typeof nested === "object" && !Array.isArray(nested)) {
    return toSosAlertRecord(nested as Record<string, unknown>);
  }
  return {
    incidentId: String(raw.incidentId ?? raw.incident_id ?? ""),
    sourceHex: normalizeHex(raw.sourceHex ?? raw.source_hex),
    conversationId: String(raw.conversationId ?? raw.conversation_id ?? ""),
    state: toSosMessageKind(raw.state),
    active: Boolean(raw.active ?? true),
    bodyUtf8: String(raw.bodyUtf8 ?? raw.body_utf8 ?? ""),
    lat: toOptionalNumber(raw.lat),
    lon: toOptionalNumber(raw.lon),
    batteryPercent: toOptionalNumber(raw.batteryPercent ?? raw.battery_percent),
    audioId: typeof raw.audioId === "string" ? raw.audioId : typeof raw.audio_id === "string" ? raw.audio_id : undefined,
    messageIdHex: toOptionalHex(raw.messageIdHex ?? raw.message_id_hex),
    receivedAtMs: Number(raw.receivedAtMs ?? raw.received_at_ms ?? Date.now()),
    updatedAtMs: Number(raw.updatedAtMs ?? raw.updated_at_ms ?? Date.now()),
  };
}

export function toSosLocationRecord(raw: Record<string, unknown>): SosLocationRecord {
  return {
    incidentId: String(raw.incidentId ?? raw.incident_id ?? ""),
    sourceHex: normalizeHex(raw.sourceHex ?? raw.source_hex),
    lat: Number(raw.lat ?? 0),
    lon: Number(raw.lon ?? 0),
    alt: toOptionalNumber(raw.alt),
    accuracy: toOptionalNumber(raw.accuracy),
    batteryPercent: toOptionalNumber(raw.batteryPercent ?? raw.battery_percent),
    recordedAtMs: Number(raw.recordedAtMs ?? raw.recorded_at_ms ?? Date.now()),
  };
}

export function toSosAudioRecord(raw: Record<string, unknown>): SosAudioRecord {
  return {
    audioId: String(raw.audioId ?? raw.audio_id ?? ""),
    incidentId: String(raw.incidentId ?? raw.incident_id ?? ""),
    sourceHex: normalizeHex(raw.sourceHex ?? raw.source_hex),
    path: String(raw.path ?? ""),
    mimeType: String(raw.mimeType ?? raw.mime_type ?? "audio/mp4"),
    durationSeconds: Number(raw.durationSeconds ?? raw.duration_seconds ?? 0),
    createdAtMs: Number(raw.createdAtMs ?? raw.created_at_ms ?? Date.now()),
  };
}

export function sosAudioToPlugin(audio: SosAudioRecord): Record<string, unknown> {
  return {
    audioId: audio.audioId,
    incidentId: audio.incidentId,
    sourceHex: audio.sourceHex,
    path: audio.path,
    mimeType: audio.mimeType,
    durationSeconds: audio.durationSeconds,
    createdAtMs: audio.createdAtMs,
  };
}

export function sosSettingsToPlugin(settings: SosSettingsRecord): Record<string, unknown> {
  return {
    enabled: settings.enabled,
    messageTemplate: settings.messageTemplate,
    cancelMessageTemplate: settings.cancelMessageTemplate,
    countdownSeconds: settings.countdownSeconds,
    includeLocation: settings.includeLocation,
    triggerShake: settings.triggerShake,
    triggerTapPattern: settings.triggerTapPattern,
    triggerPowerButton: settings.triggerPowerButton,
    shakeSensitivity: settings.shakeSensitivity,
    audioRecording: settings.audioRecording,
    audioDurationSeconds: settings.audioDurationSeconds,
    periodicUpdates: settings.periodicUpdates,
    updateIntervalSeconds: settings.updateIntervalSeconds,
    floatingButton: settings.floatingButton,
    silentAutoAnswer: settings.silentAutoAnswer,
    deactivationPinHash: settings.deactivationPinHash,
    deactivationPinSalt: settings.deactivationPinSalt,
    floatingButtonX: settings.floatingButtonX,
    floatingButtonY: settings.floatingButtonY,
    activePillX: settings.activePillX,
    activePillY: settings.activePillY,
  };
}

export function toOperationalSummary(raw: Record<string, unknown>): OperationalSummary {
  return {
    running: Boolean(raw.running),
    peerCountTotal: Number(raw.peerCountTotal ?? 0),
    savedPeerCount: Number(raw.savedPeerCount ?? 0),
    connectedPeerCount: Number(raw.connectedPeerCount ?? raw.connected_peer_count ?? 0),
    conversationCount: Number(raw.conversationCount ?? 0),
    messageCount: Number(raw.messageCount ?? 0),
    eamCount: Number(raw.eamCount ?? 0),
    eventCount: Number(raw.eventCount ?? 0),
    telemetryCount: Number(raw.telemetryCount ?? 0),
    activePropagationNodeHex: toOptionalHex(raw.activePropagationNodeHex),
    updatedAtMs: Number(raw.updatedAtMs ?? Date.now()),
  };
}

export function configToPlugin(config: NodeConfig): Record<string, unknown> {
  return {
    name: config.name,
    storageDir: config.storageDir,
    tcpClients: config.tcpClients,
    broadcast: config.broadcast,
    transportNodeEnabled: config.transportNodeEnabled,
    announceIntervalSeconds: config.announceIntervalSeconds,
    staleAfterMinutes: config.staleAfterMinutes,
    announceCapabilities: config.announceCapabilities,
    hubMode: config.hubMode,
    hubIdentityHash: config.hubIdentityHash,
    hubApiBaseUrl: config.hubApiBaseUrl,
    hubApiKey: config.hubApiKey,
    hubRefreshIntervalSeconds: config.hubRefreshIntervalSeconds,
    rnode: config.rnode,
  };
}
