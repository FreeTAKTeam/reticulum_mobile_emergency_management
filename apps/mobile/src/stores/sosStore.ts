import {
  DEFAULT_SOS_SETTINGS,
  DEFAULT_SOS_STATUS,
  type SosAlertRecord,
  type SosAudioRecord,
  type SosLocationRecord,
  type SosSettingsRecord,
  type SosStatusRecord,
  type SosTriggerSource,
} from "@reticulum/node-client";
import { defineStore } from "pinia";
import { computed, reactive, ref } from "vue";

import { useNodeStore } from "./nodeStore";

function copySettings(settings: SosSettingsRecord): SosSettingsRecord {
  return { ...settings };
}

export const useSosStore = defineStore("sos", () => {
  const nodeStore = useNodeStore();
  const settings = reactive<SosSettingsRecord>(copySettings(DEFAULT_SOS_SETTINGS));
  const status = ref<SosStatusRecord>({ ...DEFAULT_SOS_STATUS });
  const alerts = ref<SosAlertRecord[]>([]);
  const locations = ref<SosLocationRecord[]>([]);
  const audio = ref<SosAudioRecord[]>([]);
  const initialized = ref(false);
  const busy = ref(false);
  const lastError = ref("");
  let unsubs: Array<() => void> = [];

  const active = computed(() => status.value.state !== "Idle");
  const activeAlerts = computed(() => alerts.value.filter((alert) => alert.active));
  const activeConversationIds = computed(() => new Set(activeAlerts.value.map((alert) => alert.conversationId)));
  const locationsByIncident = computed(() => {
    const grouped = new Map<string, SosLocationRecord[]>();
    for (const location of locations.value) {
      const bucket = grouped.get(location.incidentId) ?? [];
      bucket.push(location);
      grouped.set(location.incidentId, bucket);
    }
    for (const bucket of grouped.values()) {
      bucket.sort((left, right) => left.recordedAtMs - right.recordedAtMs);
    }
    return grouped;
  });

  function applySettings(next: SosSettingsRecord): void {
    Object.assign(settings, copySettings(next));
  }

  async function refresh(): Promise<void> {
    try {
      const [nextSettings, nextStatus, nextAlerts, nextLocations, nextAudio] = await Promise.all([
        nodeStore.getSosSettings(),
        nodeStore.getSosStatus(),
        nodeStore.listSosAlerts(),
        nodeStore.listSosLocations(),
        nodeStore.listSosAudio(),
      ]);
      applySettings(nextSettings);
      status.value = nextStatus;
      alerts.value = nextAlerts;
      locations.value = nextLocations;
      audio.value = nextAudio;
      lastError.value = "";
    } catch (error: unknown) {
      lastError.value = error instanceof Error ? error.message : String(error);
    }
  }

  function bindEvents(): void {
    for (const unsub of unsubs) {
      unsub();
    }
    unsubs = [
      nodeStore.onClientEvent("sosStatusChanged", (event) => {
        status.value = event.status;
      }),
      nodeStore.onClientEvent("sosAlertChanged", (event) => {
        const next = alerts.value.filter((alert) =>
          alert.incidentId !== event.alert.incidentId || alert.sourceHex !== event.alert.sourceHex,
        );
        alerts.value = [event.alert, ...next];
        void refresh();
      }),
      nodeStore.onClientEvent("projectionInvalidated", (event) => {
        if (event.scope === "Sos") {
          void refresh();
        }
      }),
    ];
  }

  async function init(): Promise<void> {
    if (initialized.value) {
      return;
    }
    initialized.value = true;
    bindEvents();
    await refresh();
  }

  async function saveSettings(next: SosSettingsRecord): Promise<void> {
    busy.value = true;
    try {
      await nodeStore.setSosSettings(copySettings(next));
      applySettings(next);
      lastError.value = "";
    } catch (error: unknown) {
      lastError.value = error instanceof Error ? error.message : String(error);
      throw error;
    } finally {
      busy.value = false;
    }
  }

  async function setPin(pin?: string): Promise<void> {
    await nodeStore.setSosPin(pin);
    await refresh();
  }

  async function trigger(source: SosTriggerSource = "FloatingButton"): Promise<void> {
    busy.value = true;
    try {
      status.value = await nodeStore.triggerSos(source);
      lastError.value = "";
    } catch (error: unknown) {
      lastError.value = error instanceof Error ? error.message : String(error);
      throw error;
    } finally {
      busy.value = false;
    }
  }

  async function deactivate(pin?: string): Promise<void> {
    busy.value = true;
    try {
      status.value = await nodeStore.deactivateSos(pin);
      lastError.value = "";
    } catch (error: unknown) {
      lastError.value = error instanceof Error ? error.message : String(error);
      throw error;
    } finally {
      busy.value = false;
    }
  }

  return {
    settings,
    status,
    alerts,
    locations,
    audio,
    active,
    activeAlerts,
    activeConversationIds,
    locationsByIncident,
    busy,
    lastError,
    init,
    refresh,
    saveSettings,
    setPin,
    trigger,
    deactivate,
  };
});
