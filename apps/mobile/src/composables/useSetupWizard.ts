import { computed, reactive, shallowRef, watch } from "vue";
import { useRouter } from "vue-router";

import {
  identitySlice,
  locationSlice,
  networkSlice,
  normalizedTcpClients,
  routingSlice,
} from "../components/settings/settingsFormSlices";
import { SETTINGS_PAGE_COPY } from "../components/settings/settingsPageCopy";
import { readSosDraft, sosSettingsForSave } from "../components/settings/sosFormDraft";
import { checkSetupPermissions, requestLocationPermission } from "../services/setupPermissions";
import { useNodeStore } from "../stores/nodeStore";
import { useSosStore } from "../stores/sosStore";
import { resolveHouseholdId } from "../utils/communityStatus";
import { runDetachedStoreTask } from "../utils/detachedStoreTask";
import {
  identitySummary,
  networkSummary,
  routingSummary,
  sosSummary,
} from "../utils/settingsIndexSummaries";
import { describeSettingsSaveError, type SettingsSaveField } from "../utils/settingsSaveErrors";
import { markSetupWizardCompleted, markSetupWizardOpened } from "../utils/setupWizardState";
import { SETUP_STEPS, type SetupWizardStepId } from "./setupWizardConfig";

export type { SetupWizardStep, SetupWizardStepId } from "./setupWizardConfig";

/** Save errors that belong to a field on the Identity step. */
const IDENTITY_FIELDS: ReadonlySet<SettingsSaveField> = new Set<SettingsSaveField>([
  "householdName",
  "householdId",
  "householdCounts",
  "roleBadges",
]);

/** Settings can load after the wizard opens: follow them until the user edits a step. */
function followStore<D extends object>(draft: D, read: () => D): void {
  watch(() => JSON.stringify(read()), (_next, previous) => {
    if (JSON.stringify(draft) === previous) {
      Object.assign(draft, read());
    }
  });
}

/**
 * First-run setup: walks the Settings form pages in index order with the same draft
 * slices, validation and labels, then saves everything at once and starts the node.
 */
export function useSetupWizard() {
  const nodeStore = useNodeStore();
  const sosStore = useSosStore();
  const router = useRouter();
  const activeIndex = shallowRef(0);
  const feedback = shallowRef("");
  const saving = shallowRef(false);
  const pairingRnode = shallowRef(false);
  const errorField = shallowRef<SettingsSaveField | undefined>(undefined);

  const settings = nodeStore.settings;
  const appDestinationHex = nodeStore.status.appDestinationHex ?? "";
  const identity = reactive(identitySlice.read(settings, appDestinationHex));
  const network = reactive(networkSlice.read(settings, appDestinationHex));
  const routing = reactive(routingSlice.read(settings, appDestinationHex));
  const location = reactive(locationSlice.read(settings, appDestinationHex));
  const sos = reactive(readSosDraft(sosStore.settings));
  followStore(identity, () => identitySlice.read(nodeStore.settings, nodeStore.status.appDestinationHex ?? ""));
  followStore(network, () => networkSlice.read(nodeStore.settings, ""));
  followStore(routing, () => routingSlice.read(nodeStore.settings, ""));
  followStore(location, () => locationSlice.read(nodeStore.settings, ""));
  followStore(sos, () => readSosDraft(sosStore.settings));

  const steps = SETUP_STEPS;
  const activeStep = computed(() => steps[activeIndex.value]);

  const stepErrors = computed<Partial<Record<SetupWizardStepId, string>>>(() => ({
    identity: identitySlice.validate?.(identity),
    network: networkSlice.validate?.(network),
    location: locationSlice.validate?.(location),
  }));
  const activeStepError = computed(() => stepErrors.value[activeStep.value.id]);
  const canGoNext = computed(() => !activeStepError.value);

  const reviewRows = computed(() => [
    { label: SETTINGS_PAGE_COPY.identity.title, value: identitySummary(identity.displayName, identity.householdName) },
    {
      label: SETTINGS_PAGE_COPY.network.title,
      value: networkSummary(normalizedTcpClients(network.tcpClients).length, network.rnodeEnabled),
    },
    ...(network.rnodeEnabled
      ? [{
          label: "RNode LoRa",
          value: `${network.rnodeDisplayName || network.rnodePeripheralId || "No device selected"} · ${network.rnodeRegion} · ${network.rnodeProfile}`,
        }]
      : []),
    { label: SETTINGS_PAGE_COPY.routing.title, value: routingSummary(routing.hubMode, routing.hubIdentityHash) },
    {
      label: SETTINGS_PAGE_COPY.location.title,
      value: location.enabled ? `Every ${location.publishIntervalSeconds} s` : "Off",
    },
    {
      label: SETTINGS_PAGE_COPY.sos.title,
      value: sosSummary(sos.enabled, {
        shake: sos.triggerShake,
        powerButton: sos.triggerPowerButton,
        floatingButton: sos.floatingButton,
      }),
    },
  ]);

  // Turning SOS on during setup also offers the floating SOS button, as setup always did.
  watch(() => sos.enabled, (enabled) => {
    if (enabled) {
      sos.floatingButton = true;
    }
  });

  function open(): void {
    markSetupWizardOpened();
    runDetachedStoreTask(nodeStore, "setup", "SOS settings load", () => sosStore.init());
  }

  function goTo(id: SetupWizardStepId): void {
    activeIndex.value = Math.max(0, steps.findIndex((step) => step.id === id));
  }

  function next(): void {
    if (activeStepError.value) {
      feedback.value = activeStepError.value;
      return;
    }
    feedback.value = "";
    pairingRnode.value = false;
    activeIndex.value = Math.min(activeIndex.value + 1, steps.length - 1);
  }

  function back(): void {
    feedback.value = "";
    if (pairingRnode.value) {
      pairingRnode.value = false;
      return;
    }
    activeIndex.value = Math.max(activeIndex.value - 1, 0);
  }

  function firstInvalidStep(): SetupWizardStepId | undefined {
    return steps.find((step) => stepErrors.value[step.id])?.id;
  }

  async function saveAll(): Promise<void> {
    const current = nodeStore.settings;
    const identityPatch = identitySlice.patch(identity, current);
    const community = identityPatch.community ?? current.community;
    await nodeStore.updateSettings({
      ...networkSlice.patch(network, current),
      ...routingSlice.patch(routing, current),
      ...locationSlice.patch(location, current),
      ...identityPatch,
      community: {
        ...community,
        householdId: resolveHouseholdId(community.householdId, nodeStore.status.appDestinationHex),
      },
    });
    await sosStore.saveSettings(sosSettingsForSave(sos, sosStore.settings));
    if (location.enabled && (await checkSetupPermissions()).location !== "granted") {
      await requestLocationPermission();
    }
  }

  async function finish(): Promise<void> {
    if (saving.value) {
      return;
    }
    const invalidStep = firstInvalidStep();
    if (invalidStep) {
      goTo(invalidStep);
      feedback.value = stepErrors.value[invalidStep] ?? "";
      return;
    }
    saving.value = true;
    feedback.value = "";
    errorField.value = undefined;
    try {
      await saveAll();
      markSetupWizardCompleted();
      await nodeStore.startNode();
      await router.replace("/dashboard");
    } catch (error: unknown) {
      const failure = describeSettingsSaveError(error);
      feedback.value = failure.message;
      errorField.value = failure.field;
      if (failure.field && IDENTITY_FIELDS.has(failure.field)) {
        goTo("identity");
      }
    } finally {
      saving.value = false;
    }
  }

  return {
    activeIndex,
    activeStep,
    activeStepError,
    back,
    canGoNext,
    errorField,
    feedback,
    finish,
    identity,
    location,
    network,
    next,
    open,
    pairingRnode,
    reviewRows,
    routing,
    saving,
    sos,
    steps,
  };
}
