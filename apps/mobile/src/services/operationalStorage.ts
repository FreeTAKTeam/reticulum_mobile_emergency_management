import { Capacitor } from "@capacitor/core";

// Browser operational projections live only until this page is reloaded.
const pageValues = new Map<string, string>();

export const operationalStorage = {
  getItem(key: string): string | null {
    return Capacitor.getPlatform() === "web" ? pageValues.get(key) ?? null : localStorage.getItem(key);
  },
  setItem(key: string, value: string): void {
    if (Capacitor.getPlatform() === "web") pageValues.set(key, value);
    else localStorage.setItem(key, value);
  },
  removeItem(key: string): void {
    if (Capacitor.getPlatform() === "web") pageValues.delete(key);
    else localStorage.removeItem(key);
  },
};
