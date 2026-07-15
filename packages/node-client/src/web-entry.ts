import type {
  ReticulumNodeClient,
  ReticulumNodeClientFactoryOptions,
} from "./contracts";
import { WebReticulumNodeClient } from "./web-client";
import { decodeBase64ToBytes, encodeBytesToBase64 } from "./runtime-converters";

export * from "./contracts";
export { DEFAULT_NODE_CONFIG, generateDefaultCallSign } from "./client-defaults";
export { DEFAULT_SOS_SETTINGS, DEFAULT_SOS_STATUS } from "./client-config-converters";
export { normalizeRnodeSettings, parseRnodeConnectionMode } from "./converters";

export function createReticulumNodeClient(
  _options: ReticulumNodeClientFactoryOptions = {},
): ReticulumNodeClient {
  return new WebReticulumNodeClient();
}

export function bytesToBase64(bytes: Uint8Array): string {
  return encodeBytesToBase64(bytes);
}

export function base64ToBytes(base64: string): Uint8Array {
  return decodeBase64ToBytes(base64);
}
