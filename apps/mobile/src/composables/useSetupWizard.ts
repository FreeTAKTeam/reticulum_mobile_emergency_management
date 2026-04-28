import { computed, reactive, shallowRef } from "vue";
import { useRouter } from "vue-router";

import { useNodeStore } from "../stores/nodeStore";
import { useSosStore } from "../stores/sosStore";
import { normalizeDisplayName } from "../utils/peers";
import { DEFAULT_TCP_COMMUNITY_ENDPOINTS, normalizeTcpCommunityClients } from "../utils/tcpCommunityServers";
import { markSetupWizardCompleted, markSetupWizardOpened } from "../utils/setupWizardState";
import {
  checkSetupPermissions,
  requestLocationPermission,
  requestNotificationPermission,
  type SetupPermissionSnapshot,
  type SetupPermissionState,
} from "../services/setupPermissions";

export type SetupWizardStepId =
  | "welcome"
  | "callsign"
  | "tcp"
  | "telemetry"
  | "permissions"
  | "sos"
  | "review";

export interface SetupWizardStep {
  id: SetupWizardStepId;
  label: string;
  title: string;
}

const SETUP_STEPS: SetupWizardStep[] = [
  { id: "welcome", label: "Welcome", title: "Welcome to R.E.M." },
  { id: "callsign", label: "Call Sign", title: "Set your call sign" },
  { id: "tcp", label: "TCP", title: "Choose TCP interfaces" },
  { id: "telemetry", label: "Telemetry", title: "Telemetry sharing" },
  { id: "permissions", label: "Permits", title: "Android permissions" },
  { id: "sos", label: "SOS", title: "SOS emergency access" },
  { id: "review", label: "Review", title: "Review setup" },
];

export function normalizeWizardTcpEndpoint(value: string): string | undefined {
  const candidate = value.trim();
  if (!candidate) {
    return undefined;
  }

  if (candidate.startsWith("[")) {
    const ipv6Match = candidate.match(/^\[[^\]]+\]:(\d{1,5})$/);
    if (!ipv6Match) {
      return undefined;
    }
    const port = Number(ipv6Match[1]);
    return Number.isInteger(port) && port >= 1 && port <= 65535 ? candidate : undefined;
  }

  const separatorIndex = candidate.lastIndexOf(":");
  if (separatorIndex <= 0 || separatorIndex === candidate.length - 1) {
    return undefined;
  }

  const host = candidate.slice(0, separatorIndex).trim();
  const port = Number(candidate.slice(separatorIndex + 1).trim());
  if (!host || !Number.isInteger(port) || port < 1 || port > 65535) {
    return undefined;
  }
  return `${host}:${port}`;
}

export function useSetupWizard() {
  const nodeStore = useNodeStore();
  const sosStore = useSosStore();
  const router = useRouter();
  const activeIndex = shallowRef(0);
  const customTcpEndpoint = shallowRef("");
  const feedback = shallowRef("");
  const saving = shallowRef(false);
  const permissions = reactive<SetupPermissionSnapshot>({
    location: "prompt",
    notifications: "prompt",
  });

  const draft = reactive({
    displayName: nodeStore.settings.displayName,
    tcpClients: [...nodeStore.settings.tcpClients],
    telemetryEnabled: nodeStore.settings.telemetry.enabled,
    sosEnabled: sosStore.settings.enabled,
  });

  const steps = SETUP_STEPS;
  const activeStep = computed(() => steps[activeIndex.value]);
  const normalizedDisplayName = computed(() => normalizeDisplayName(draft.displayName) ?? "");
  const normalizedTcpClients = computed(() => normalizeTcpCommunityClients(draft.tcpClients, DEFAULT_TCP_COMMUNITY_ENDPOINTS));
  const selectedTcpEndpointSet = computed(() => new Set(normalizedTcpClients.value));
  const sosFloatingButtonEnabled = computed(() => draft.sosEnabled || sosStore.settings.floatingButton);

  const canGoNext = computed(() => {
    if (activeStep.value.id === "callsign") {
      return normalizedDisplayName.value.length > 0;
    }
    return true;
  });

  function open(): void {
    markSetupWizardOpened();
    void refreshPermissions();
  }

  async function refreshPermissions(): Promise<void> {
    const snapshot = await checkSetupPermissions();
    permissions.location = snapshot.location;
    permissions.notifications = snapshot.notifications;
  }

  function setTcpEndpoint(endpoint: string, selected: boolean): void {
    const next = new Set(normalizedTcpClients.value);
    if (selected) {
      next.add(endpoint);
    } else {
      next.delete(endpoint);
    }
    draft.tcpClients = [...next];
  }

  function addCustomTcpEndpoint(): void {
    const normalized = normalizeWizardTcpEndpoint(customTcpEndpoint.value);
    if (!normalized) {
      feedback.value = "Invalid endpoint. Use host:port or [ipv6]:port.";
      return;
    }
    const next = new Set(normalizedTcpClients.value);
    next.add(normalized);
    draft.tcpClients = [...next];
    customTcpEndpoint.value = "";
    feedback.value = "";
  }

  function removeTcpEndpoint(endpoint: string): void {
    draft.tcpClients = normalizedTcpClients.value.filter((entry) => entry !== endpoint);
  }

  function next(): void {
    if (!canGoNext.value) {
      feedback.value = "Set a call sign before continuing.";
      return;
    }
    feedback.value = "";
    activeIndex.value = Math.min(activeIndex.value + 1, steps.length - 1);
  }

  function back(): void {
    feedback.value = "";
    activeIndex.value = Math.max(activeIndex.value - 1, 0);
  }

  async function requestLocation(): Promise<void> {
    permissions.location = await requestLocationPermission();
  }

  async function requestNotifications(): Promise<void> {
    permissions.notifications = await requestNotificationPermission();
  }

  function permissionLabel(value: SetupPermissionState): string {
    switch (value) {
      case "granted":
        return "Granted";
      case "denied":
        return "Denied";
      case "unavailable":
        return "Unavailable";
      case "prompt":
      default:
        return "Not requested";
    }
  }

  async function finish(): Promise<void> {
    if (!normalizedDisplayName.value || saving.value) {
      feedback.value = "Set a call sign before finishing setup.";
      return;
    }
    saving.value = true;
    feedback.value = "";
    try {
      nodeStore.updateSettings({
        displayName: normalizedDisplayName.value,
        tcpClients: normalizedTcpClients.value,
        telemetry: {
          ...nodeStore.settings.telemetry,
          enabled: draft.telemetryEnabled,
        },
      });
      await sosStore.saveSettings({
        ...sosStore.settings,
        enabled: draft.sosEnabled,
        floatingButton: draft.sosEnabled ? true : sosStore.settings.floatingButton,
      });
      if (draft.telemetryEnabled && permissions.location !== "granted") {
        permissions.location = await requestLocationPermission();
      }
      markSetupWizardCompleted();
      await router.replace("/dashboard");
    } catch (error: unknown) {
      feedback.value = error instanceof Error ? error.message : String(error);
    } finally {
      saving.value = false;
    }
  }

  return {
    activeIndex,
    activeStep,
    canGoNext,
    customTcpEndpoint,
    draft,
    feedback,
    normalizedDisplayName,
    normalizedTcpClients,
    open,
    permissions,
    permissionLabel,
    refreshPermissions,
    saving,
    selectedTcpEndpointSet,
    sosFloatingButtonEnabled,
    steps,
    addCustomTcpEndpoint,
    back,
    finish,
    next,
    removeTcpEndpoint,
    requestLocation,
    requestNotifications,
    setTcpEndpoint,
  };
}
