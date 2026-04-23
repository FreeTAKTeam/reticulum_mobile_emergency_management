import { registerPlugin } from "@capacitor/core";

export type TelemetryLocationPermissionState = "granted" | "denied" | "prompt" | "prompt-with-rationale";

export interface TelemetryLocationPermissionStatus {
  location: TelemetryLocationPermissionState;
  coarseLocation: TelemetryLocationPermissionState;
}

export interface TelemetryLocationOptions {
  enableHighAccuracy?: boolean;
  timeout?: number;
  maximumAge?: number;
}

export interface TelemetryLocationCoordinates {
  latitude: number;
  longitude: number;
  accuracy: number;
  altitude?: number;
  heading?: number;
  speed?: number;
}

export interface TelemetryLocationPosition {
  coords: TelemetryLocationCoordinates;
  timestamp: number;
}

export interface TelemetryLocationPlugin {
  checkPermissions(): Promise<TelemetryLocationPermissionStatus>;
  requestPermissions(): Promise<TelemetryLocationPermissionStatus>;
  getCurrentPosition(options?: TelemetryLocationOptions): Promise<TelemetryLocationPosition>;
}

export const telemetryLocationPlugin = registerPlugin<TelemetryLocationPlugin>("TelemetryLocation");
