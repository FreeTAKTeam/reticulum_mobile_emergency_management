import {
  type LogLevel,
  type NodeErrorEvent,
  type NodeStatus,
  type ReticulumNodeClient,
} from "@reticulum/node-client";
import type { Ref, ShallowRef } from "vue";

import type { NodeUiSettings } from "../types/domain";
import { nativeLogShouldAppendToUi } from "../utils/nativeUiBackpressure";
import {
  hasConfiguredNonTcpInterface,
  logIndicatesTcpInterfaceReadinessError,
  nodeErrorIndicatesTcpInterfaceReadinessError,
  summarizeRnodeInterfaceState,
} from "../utils/readinessErrors";
import { runtimeProfile } from "../utils/runtimeProfile";
import { DEFAULT_TCP_COMMUNITY_ENDPOINTS } from "../utils/tcpCommunityServers";
import {
  DEFAULT_SETTINGS,
  storeNodeConfigRestartRequired,
} from "./nodeSettingsModel";
import {
  type UiLogLine,
  asTrimmedString,
  nowMs,
} from "./nodeStoreCore";

interface NodeLoggingContext {
  client: ShallowRef<ReticulumNodeClient | null>;
  lastError: Ref<string>;
  logs: Ref<UiLogLine[]>;
  nodeConfigRestartRequired: Ref<boolean>;
  nodeControlEntries: Ref<UiLogLine[]>;
  readinessError: Ref<string>;
  settings: NodeUiSettings;
  status: Ref<NodeStatus>;
}

export function createNodeLoggingController(context: NodeLoggingContext) {
  const {
    client,
    lastError,
    logs,
    nodeConfigRestartRequired,
    nodeControlEntries,
    readinessError,
    settings,
    status,
  } = context;
  let lastRnodeInterfaceFingerprint = "";
  let lastRnodeBlockingMessage = "";

  function defaultsWithTcpFallback(): string[] {
    return DEFAULT_SETTINGS.tcpClients.length > 0
      ? [...DEFAULT_SETTINGS.tcpClients]
      : [...DEFAULT_TCP_COMMUNITY_ENDPOINTS];
  }

  function appendLog(level: string, message: string): void {
    logs.value = [{ at: nowMs(), level, message }, ...logs.value].slice(0, 120);
  }

  function appendNodeControlEntry(level: string, message: string, at = nowMs()): void {
    nodeControlEntries.value = [{ at, level, message }, ...nodeControlEntries.value].slice(0, 120);
  }

  function setNodeConfigRestartRequired(required: boolean): void {
    nodeConfigRestartRequired.value = required;
    storeNodeConfigRestartRequired(required);
  }

  function toPluginLogLevel(level: string): LogLevel {
    switch (asTrimmedString(level).toLowerCase()) {
      case "trace":
        return "Trace";
      case "debug":
        return "Debug";
      case "warn":
        return "Warn";
      case "error":
        return "Error";
      case "info":
      default:
        return "Info";
    }
  }

  function mirrorUiLogToNative(level: string, message: string): void {
    if (!client.value || runtimeProfile === "web") {
      return;
    }
    const normalizedLevel = asTrimmedString(level).toLowerCase();
    if (normalizedLevel !== "warn" && normalizedLevel !== "error") {
      return;
    }
    void client.value.logMessage(toPluginLogLevel(level), message).catch(() => undefined);
  }

  function logUi(level: string, message: string): void {
    appendLog(level, message);
    mirrorUiLogToNative(level, message);
    const normalizedLevel = asTrimmedString(level).toLowerCase();
    if (normalizedLevel === "error") {
      lastError.value = message;
      console.error(`[ui][${level}] ${message}`);
      return;
    }
    if (normalizedLevel === "debug" || normalizedLevel === "trace") {
      console.debug(`[ui][${level}] ${message}`);
      return;
    }
    if (normalizedLevel === "warn") {
      console.warn(`[ui][${level}] ${message}`);
      return;
    }
    console.info(`[ui][${level}] ${message}`);
  }

  function setLastError(message: string): void {
    lastError.value = asTrimmedString(message);
  }

  function clearLastError(): void {
    lastError.value = "";
  }

  function messageIsCurrentReadinessError(message: string, currentReadinessError: string): boolean {
    const trimmed = asTrimmedString(message);
    if (!trimmed) {
      return false;
    }
    return trimmed === currentReadinessError
      || trimmed === `Node marked not ready: ${currentReadinessError}`;
  }

  function clearReadinessError(): void {
    const previous = asTrimmedString(readinessError.value);
    readinessError.value = "";
    if (!previous) {
      return;
    }
    if (messageIsCurrentReadinessError(lastError.value, previous)) {
      clearLastError();
    }
    nodeControlEntries.value = nodeControlEntries.value.filter(
      (entry) => !messageIsCurrentReadinessError(entry.message, previous),
    );
  }

  function setReadinessError(message: string, at = nowMs()): void {
    const trimmed = asTrimmedString(message);
    if (!trimmed) {
      return;
    }
    const wasReady = !asTrimmedString(readinessError.value);
    readinessError.value = trimmed;
    lastError.value = trimmed;
    if (wasReady) {
      appendNodeControlEntry("Error", `Node marked not ready: ${trimmed}`, at);
    }
  }

  function tcpInterfaceFailureCanFallBackToConfiguredInterface(message: string): boolean {
    return hasConfiguredNonTcpInterface(settings)
      && logIndicatesTcpInterfaceReadinessError(message);
  }

  function nodeErrorCanFallBackToConfiguredInterface(event: NodeErrorEvent): boolean {
    return hasConfiguredNonTcpInterface(settings)
      && nodeErrorIndicatesTcpInterfaceReadinessError(event);
  }

  function applyRnodeInterfaceReadiness(at = nowMs()): void {
    const summary = summarizeRnodeInterfaceState(status.value, settings);
    const message = asTrimmedString(summary.message);
    const fingerprint = [
      summary.severity,
      message,
      summary.rnodeAvailable ? "rnode-rx" : "rnode-no-rx",
      summary.otherAvailableCount,
    ].join("|");
    const previousBlockingMessage = lastRnodeBlockingMessage;
    const fingerprintChanged = fingerprint !== lastRnodeInterfaceFingerprint;

    if (summary.severity === "blocking") {
      if (message) {
        setReadinessError(message, at);
        if (fingerprintChanged && previousBlockingMessage && previousBlockingMessage !== message) {
          appendNodeControlEntry("Warn", message, at);
        }
        lastRnodeBlockingMessage = message;
      }
    } else {
      if (readinessError.value) {
        clearReadinessError();
      }
      lastRnodeBlockingMessage = "";
    }

    if (!fingerprintChanged) {
      return;
    }
    lastRnodeInterfaceFingerprint = fingerprint;

    if (summary.severity === "degraded" && message) {
      appendNodeControlEntry("Warn", message, at);
    } else if (summary.severity === "ready" && summary.rnodeConfigured) {
      const otherInterfaceText = summary.otherAvailableCount === 0
        ? "no other receiving interfaces"
        : `${summary.otherAvailableCount} other receiving interface${summary.otherAvailableCount === 1 ? "" : "s"}`;
      appendNodeControlEntry(
        "Info",
        `RNode LoRa available with ${otherInterfaceText}.`,
        at,
      );
    }
  }

  function errorMessage(error: unknown): string {
    if (error instanceof Error) {
      return error.message;
    }
    return String(error);
  }

  function captureActionError(action: string, error: unknown): Error {
    const message = `${action}: ${errorMessage(error)}`;
    lastError.value = message;
    mirrorUiLogToNative("Error", message);
    console.error(`[ui][Error] ${message}`);
    appendLog("Error", message);
    return error instanceof Error ? error : new Error(message);
  }

  function captureRuntimeActionError(action: string, error: unknown): Error {
    const message = `${action}: ${errorMessage(error)}`;
    const captured = captureActionError(action, error);
    setReadinessError(message);
    return captured;
  }

  return {
    appendLog,
    appendNodeControlEntry,
    applyRnodeInterfaceReadiness,
    captureActionError,
    captureRuntimeActionError,
    clearLastError,
    clearReadinessError,
    defaultsWithTcpFallback,
    errorMessage,
    logUi,
    mirrorUiLogToNative,
    nodeErrorCanFallBackToConfiguredInterface,
    setLastError,
    setNodeConfigRestartRequired,
    setReadinessError,
    tcpInterfaceFailureCanFallBackToConfiguredInterface,
    toPluginLogLevel,
  };
}
