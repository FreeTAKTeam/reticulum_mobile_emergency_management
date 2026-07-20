export const NODE_ERROR_CODES = [
  "InvalidConfig",
  "IoError",
  "NetworkError",
  "ReticulumError",
  "AlreadyRunning",
  "NotRunning",
  "Timeout",
  "LxmfWireEncodeError",
  "LxmfMessageIdParseError",
  "LxmfPacketTooLarge",
  "LxmfPacketBuildError",
  "EventStreamClosed",
  "InternalError",
  "NativeError",
  "UnknownError",
] as const;

export type NodeErrorCode = (typeof NODE_ERROR_CODES)[number];

export interface NodeErrorDetails {
  code?: unknown;
  message?: unknown;
  operation?: unknown;
  retryable?: unknown;
  cause?: unknown;
}

const RETRYABLE_CODES = new Set<NodeErrorCode>([
  "IoError",
  "NetworkError",
  "ReticulumError",
  "Timeout",
  "EventStreamClosed",
]);

function asRecord(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : undefined;
}

function asNonEmptyString(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}

function normalizedCode(value: unknown): NodeErrorCode {
  const candidate = asNonEmptyString(value);
  return NODE_ERROR_CODES.includes(candidate as NodeErrorCode)
    ? candidate as NodeErrorCode
    : "UnknownError";
}

export class ReticulumNodeError extends Error {
  readonly code: NodeErrorCode;
  readonly operation?: string;
  readonly retryable: boolean;
  override readonly cause?: unknown;

  constructor(details: NodeErrorDetails, fallbackOperation?: string) {
    const code = normalizedCode(details.code);
    const message = asNonEmptyString(details.message) ?? "Reticulum node operation failed.";
    const operation = asNonEmptyString(details.operation) ?? asNonEmptyString(fallbackOperation);
    super(message);
    this.name = "ReticulumNodeError";
    this.code = code;
    this.operation = operation;
    this.retryable = typeof details.retryable === "boolean"
      ? details.retryable
      : RETRYABLE_CODES.has(code);
    this.cause = details.cause;
  }
}

export function classifyNodeError(error: unknown, operation?: string): ReticulumNodeError {
  if (error instanceof ReticulumNodeError) {
    return error;
  }

  const record = asRecord(error);
  const data = asRecord(record?.data);
  return new ReticulumNodeError({
    code: data?.code ?? record?.code,
    message: data?.message ?? record?.message ?? String(error),
    operation: data?.operation,
    retryable: data?.retryable,
    cause: data?.cause ?? error,
  }, operation);
}

export function classifyPluginErrors<T extends object>(plugin: T): T {
  return new Proxy(plugin, {
    get(target, property, receiver) {
      const value = Reflect.get(target, property, receiver) as unknown;
      if (typeof value !== "function") {
        return value;
      }

      return (...args: unknown[]) => {
        try {
          const result = Reflect.apply(value, target, args) as unknown;
          if (result && typeof (result as PromiseLike<unknown>).then === "function") {
            return Promise.resolve(result).catch((error: unknown) => {
              throw classifyNodeError(error, String(property));
            });
          }
          return result;
        } catch (error: unknown) {
          throw classifyNodeError(error, String(property));
        }
      };
    },
  });
}
