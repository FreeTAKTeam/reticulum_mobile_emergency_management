import { registerPlugin } from "@capacitor/core";

type PermissionState = "prompt" | "prompt-with-rationale" | "granted" | "denied";

export interface TelemetryLocationPermissionStatus {
  location: PermissionState;
  coarseLocation: PermissionState;
}

export interface TelemetryLocationOptions {
  enableHighAccuracy?: boolean;
  timeout?: number;
  maximumAge?: number;
}

export interface TelemetryLocationPosition {
  coords: {
    latitude: number;
    longitude: number;
    accuracy: number;
    altitude?: number | null;
    heading?: number | null;
    speed?: number | null;
  };
  timestamp: number;
}

interface TelemetryLocationPlugin {
  checkPermissions(): Promise<TelemetryLocationPermissionStatus>;
  requestPermissions(): Promise<TelemetryLocationPermissionStatus>;
  getCurrentPosition(options?: TelemetryLocationOptions): Promise<TelemetryLocationPosition>;
}

export const telemetryLocationPlugin = registerPlugin<TelemetryLocationPlugin>("TelemetryLocation");
