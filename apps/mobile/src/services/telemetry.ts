import { Capacitor } from "@capacitor/core";

import { telemetryLocationPlugin } from "./telemetryLocationPlugin";

export interface TelemetryFix {
  lat: number;
  lon: number;
  alt?: number;
  course?: number;
  speed?: number;
  accuracy?: number;
  timestamp: number;
}

export type TelemetryPermissionState = "granted" | "denied" | "prompt" | "unavailable";

export class TelemetryUnavailableError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "TelemetryUnavailableError";
  }
}

export class TelemetryPermissionDeniedError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "TelemetryPermissionDeniedError";
  }
}

function toFixFromWeb(position: GeolocationPosition): TelemetryFix {
  return {
    lat: position.coords.latitude,
    lon: position.coords.longitude,
    alt: position.coords.altitude ?? undefined,
    course: position.coords.heading ?? undefined,
    speed: position.coords.speed ?? undefined,
    accuracy: position.coords.accuracy ?? undefined,
    timestamp: position.timestamp,
  };
}

function normalizeError(error: unknown): Error {
  if (error instanceof Error) {
    return error;
  }
  return new Error(String(error));
}

function isPermissionDenied(error: unknown): boolean {
  const normalized = normalizeError(error);
  const message = normalized.message.toLowerCase();
  return message.includes("denied") || message.includes("not allowed") || message.includes("permission");
}

export class TelemetryService {
  async getPermissionState(): Promise<TelemetryPermissionState> {
    if (Capacitor.isNativePlatform()) {
      try {
        const permissions = await telemetryLocationPlugin.checkPermissions();
        const location = permissions.location;
        if (location === "granted") {
          return "granted";
        }
        if (location === "denied") {
          return "denied";
        }
        return "prompt";
      } catch {
        return "unavailable";
      }
    }

    if (!("geolocation" in navigator)) {
      return "unavailable";
    }

    if (!("permissions" in navigator)) {
      return "prompt";
    }

    try {
      const permission = await navigator.permissions.query({ name: "geolocation" });
      if (permission.state === "granted") {
        return "granted";
      }
      if (permission.state === "denied") {
        return "denied";
      }
      return "prompt";
    } catch {
      return "prompt";
    }
  }

  async requestPermission(): Promise<TelemetryPermissionState> {
    if (Capacitor.isNativePlatform()) {
      try {
        const permissions = await telemetryLocationPlugin.requestPermissions();
        const location = permissions.location;
        if (location === "granted") {
          return "granted";
        }
        if (location === "denied") {
          return "denied";
        }
        return "prompt";
      } catch {
        return "unavailable";
      }
    }

    try {
      await this.getCurrentPosition();
      return "granted";
    } catch (error: unknown) {
      if (error instanceof TelemetryPermissionDeniedError) {
        return "denied";
      }
      return "prompt";
    }
  }

  async getCurrentPosition(): Promise<TelemetryFix> {
    if (Capacitor.isNativePlatform()) {
      try {
        const position = await telemetryLocationPlugin.getCurrentPosition({
          enableHighAccuracy: true,
          timeout: 15000,
          maximumAge: 5000,
        });
        return {
          lat: position.coords.latitude,
          lon: position.coords.longitude,
          alt: position.coords.altitude ?? undefined,
          course: position.coords.heading ?? undefined,
          speed: position.coords.speed ?? undefined,
          accuracy: position.coords.accuracy ?? undefined,
          timestamp: position.timestamp,
        };
      } catch (error: unknown) {
        if (isPermissionDenied(error)) {
          throw new TelemetryPermissionDeniedError("Location permission denied.");
        }
        throw new TelemetryUnavailableError("Unable to read device location.");
      }
    }

    if (!("geolocation" in navigator)) {
      throw new TelemetryUnavailableError("Geolocation API is unavailable on this device.");
    }

    return new Promise<TelemetryFix>((resolve, reject) => {
      navigator.geolocation.getCurrentPosition(
        (position) => resolve(toFixFromWeb(position)),
        (error) => {
          if (error.code === error.PERMISSION_DENIED) {
            reject(new TelemetryPermissionDeniedError("Location permission denied."));
            return;
          }
          reject(new TelemetryUnavailableError("Unable to read browser geolocation."));
        },
        {
          enableHighAccuracy: true,
          timeout: 15000,
          maximumAge: 5000,
        },
      );
    });
  }
}

export const telemetryService = new TelemetryService();
