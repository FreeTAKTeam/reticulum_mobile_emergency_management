export type RuntimeProfile = "web" | "mobile";

function normalizeRuntimeProfile(value: string | undefined): RuntimeProfile {
  return value === "mobile" ? "mobile" : "web";
}

export const runtimeProfile = normalizeRuntimeProfile(
  import.meta.env.VITE_RUNTIME_PROFILE,
);

export const isWebRuntimeProfile = runtimeProfile === "web";
export const supportsNativeNodeRuntime = runtimeProfile === "mobile";
