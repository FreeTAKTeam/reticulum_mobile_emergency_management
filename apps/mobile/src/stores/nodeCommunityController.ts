import type {
  BlockOnboardingDraft,
  BlockOnboardingImportRequest,
  BlockOnboardingImportResult,
  BlockOnboardingInspection,
  HouseholdStatus,
  NodeStatus,
  PowerStateRecord,
  ReticulumNodeClient,
  SignedBlockOnboardingEnvelope,
} from "@reticulum/node-client";
import type { Ref, ShallowRef } from "vue";

import type { NodeUiSettings } from "../types/domain";

interface NodeCommunityContext {
  client: ShallowRef<ReticulumNodeClient | null>;
  init: () => Promise<void>;
  powerState: Ref<PowerStateRecord>;
  publishSettings: (next: Partial<NodeUiSettings>) => Promise<void>;
  refreshSavedPeersProjection: () => Promise<void>;
  refreshSettingsProjection: () => Promise<void>;
  settings: NodeUiSettings;
  status: Ref<NodeStatus>;
}

export function createNodeCommunityController(context: NodeCommunityContext) {
  async function nativeClient(operation: string): Promise<ReticulumNodeClient> {
    await context.init();
    if (!context.client.value || !context.status.value.running) {
      throw new Error(`${operation} requires the native Reticulum node.`);
    }
    return context.client.value;
  }

  async function refreshPowerState(): Promise<PowerStateRecord> {
    const client = await nativeClient("Power-state refresh");
    context.powerState.value = await client.getPowerState();
    return { ...context.powerState.value };
  }

  async function publishCommunityStatus(status: HouseholdStatus): Promise<void> {
    await context.publishSettings({
      community: { ...context.settings.community, status },
    });
    const client = await nativeClient("Community status publishing");
    await client.publishCommunityStatus();
  }

  async function createBlockOnboardingCode(
    draft: BlockOnboardingDraft,
  ): Promise<SignedBlockOnboardingEnvelope> {
    return (await nativeClient("Block Code creation")).createBlockOnboardingCode(draft);
  }

  async function inspectBlockOnboardingCode(
    encodedText: string,
  ): Promise<BlockOnboardingInspection> {
    return (await nativeClient("Block Code inspection")).inspectBlockOnboardingCode(encodedText);
  }

  async function importBlockOnboardingCode(
    request: BlockOnboardingImportRequest,
  ): Promise<BlockOnboardingImportResult> {
    const result = await (await nativeClient("Block Code import"))
      .importBlockOnboardingCode(request);
    await Promise.all([
      context.refreshSettingsProjection(),
      context.refreshSavedPeersProjection(),
    ]);
    return result;
  }

  return {
    createBlockOnboardingCode,
    importBlockOnboardingCode,
    inspectBlockOnboardingCode,
    publishCommunityStatus,
    refreshPowerState,
  };
}
