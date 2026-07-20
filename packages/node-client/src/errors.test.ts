import { describe, expect, it } from "vitest";

import {
  ReticulumNodeError,
  classifyNodeError,
  classifyPluginErrors,
} from "./errors";

describe("ReticulumNodeError", () => {
  it("preserves native classification details", () => {
    const cause = new Error("socket closed");
    const error = classifyNodeError({
      code: "NetworkError",
      message: "peer connection failed",
      data: {
        operation: "connectPeer",
        retryable: true,
        cause,
      },
    });

    expect(error).toBeInstanceOf(ReticulumNodeError);
    expect(error.code).toBe("NetworkError");
    expect(error.operation).toBe("connectPeer");
    expect(error.retryable).toBe(true);
    expect(error.cause).toBe(cause);
  });

  it("classifies unknown failures without losing their cause", () => {
    const cause = new Error("unexpected bridge failure");
    const error = classifyNodeError(cause, "getStatus");

    expect(error.code).toBe("UnknownError");
    expect(error.message).toBe("unexpected bridge failure");
    expect(error.operation).toBe("getStatus");
    expect(error.retryable).toBe(false);
    expect(error.cause).toBe(cause);
  });

  it("classifies rejected plugin promises at the operation boundary", async () => {
    const plugin = classifyPluginErrors({
      async send(): Promise<void> {
        throw {
          message: "send timed out",
          code: "Timeout",
          data: { retryable: true },
        };
      },
    });

    await expect(plugin.send()).rejects.toMatchObject({
      name: "ReticulumNodeError",
      code: "Timeout",
      operation: "send",
      retryable: true,
    });
  });
});
