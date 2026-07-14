import { describe, expect, it } from "vitest";

import { base64ToBytes, bytesToBase64 } from "./index";

describe("binary Base64 helpers", () => {
  it("round-trips arbitrary binary payloads", () => {
    const bytes = Uint8Array.from([0, 1, 2, 127, 128, 254, 255]);

    expect(base64ToBytes(bytesToBase64(bytes))).toEqual(bytes);
  });

  it("uses the stable RFC 4648 representation", () => {
    expect(bytesToBase64(Uint8Array.from([0x52, 0x45, 0x4d]))).toBe("UkVN");
    expect(base64ToBytes("UkVN")).toEqual(Uint8Array.from([0x52, 0x45, 0x4d]));
  });
});
