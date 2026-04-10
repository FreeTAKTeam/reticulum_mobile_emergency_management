import { Capacitor } from "@capacitor/core";

export type RuntimeProfile = "web" | "mobile";

function inferNativeRuntimeProfile(): RuntimeProfile {
  return Capacitor.getPlatform() === "web" ? "web" : "mobile";
}

function normalizeRuntimeProfile(value: string | undefined): RuntimeProfile {
  if (value === "mobile" || value === "web") {
    return value;
  }
  return inferNativeRuntimeProfile();
}

export const runtimeProfile = normalizeRuntimeProfile(import.meta.env.VITE_RUNTIME_PROFILE);

export const isWebRuntimeProfile = runtimeProfile === "web";
export const supportsNativeNodeRuntime = runtimeProfile === "mobile";
