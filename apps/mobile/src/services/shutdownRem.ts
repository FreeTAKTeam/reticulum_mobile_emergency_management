import { App } from "@capacitor/app";
import { Capacitor } from "@capacitor/core";

export async function shutdownRem(stopNode: () => Promise<void>): Promise<void> {
  await stopNode();
  if (Capacitor.getPlatform() === "android") {
    await App.exitApp();
  }
}
