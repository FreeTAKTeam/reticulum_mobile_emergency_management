import { expect, test, vi } from "vitest";
import { randomHex32 } from "./client-defaults";

test("mock identities use sixteen cryptographically random bytes", () => {
  const random = vi.spyOn(crypto, "getRandomValues").mockImplementation((bytes) => {
    (bytes as Uint8Array).set(Array.from({ length: 16 }, (_, index) => index));
    return bytes;
  });
  try {
    expect(randomHex32()).toBe("000102030405060708090a0b0c0d0e0f");
    expect(random).toHaveBeenCalledOnce();
  } finally {
    random.mockRestore();
  }
});
