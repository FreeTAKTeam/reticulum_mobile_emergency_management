export type LocalPropagationState =
  | "StoredLocally" | "Uploading" | "UploadedUnverified" | "RelayHolds"
  | "Delivered" | "Acknowledged" | "Expired" | "Evicted";

export interface LocalPropagationRecord {
  transientIdHex: string;
  messageIdHex: string;
  destinationHex: string;
  state: LocalPropagationState;
  encryptedBytes: number;
  admittedAtMs: number;
  expiresAtMs: number;
  mayExistOnRelay: boolean;
}

export interface LocalPropagationConfig {
  capacityBytes: number;
  retentionSeconds: number;
  loraPollIntervalSeconds: number;
  loraTransferLimitBytes: number;
}
