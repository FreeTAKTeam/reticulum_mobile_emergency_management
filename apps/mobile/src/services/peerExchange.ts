import { Capacitor } from "@capacitor/core";
import { Clipboard } from "@capacitor/clipboard";
import { Share } from "@capacitor/share";

export async function copyToClipboard(text: string): Promise<void> {
  if (Capacitor.isNativePlatform()) {
    await Clipboard.write({ string: text });
    return;
  }

  if (typeof navigator !== "undefined" && navigator.clipboard) {
    await navigator.clipboard.writeText(text);
    return;
  }

  throw new Error("Clipboard API is not available on this platform.");
}

export async function shareText(title: string, text: string): Promise<void> {
  if (Capacitor.isNativePlatform()) {
    await Share.share({
      title,
      text,
      dialogTitle: title,
    });
    return;
  }

  downloadTextFile(`${title.replace(/\s+/g, "_")}.json`, text, "application/json");
}

export function downloadTextFile(
  fileName: string,
  text: string,
  mimeType: string,
): void {
  const blob = new Blob([text], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = fileName;
  anchor.click();
  URL.revokeObjectURL(url);
}
