import type { PluginCapabilityRecord } from "@reticulum/node-client";

/** Plain-language names for the REM host capabilities a plugin can be granted. */
export const PLUGIN_CAPABILITY_DEFINITIONS: Array<{
  key: keyof PluginCapabilityRecord;
  label: string;
}> = [
  { key: "eventsPublish", label: "Publish events" },
  { key: "sensorsPublish", label: "Publish sensors" },
  { key: "lxmfSend", label: "Send LXMF" },
  { key: "lxmfReceive", label: "Receive LXMF" },
  { key: "notificationsRaise", label: "Raise notifications" },
  { key: "operationalRead", label: "Read operational status" },
  { key: "mapMarkersRead", label: "Read its own map markers" },
  { key: "mapMarkersWrite", label: "Add and edit its own map markers" },
  { key: "mapObserverRead", label: "Read this device's location" },
  { key: "mapSelectionRead", label: "Read map objects you share with it" },
  { key: "mapNearbyRead", label: "Read all map markers near this device" },
];

/** Plain words for the Android permissions a plugin app uses; anything else stays in the technical details. */
const ANDROID_PERMISSION_LABELS: Array<[RegExp, string]> = [
  [/BLUETOOTH/, "Bluetooth"],
  [/LOCATION/, "location"],
  [/POST_NOTIFICATIONS/, "notifications"],
  [/FOREGROUND_SERVICE/, "running in the background"],
  [/CAMERA/, "camera"],
  [/RECORD_AUDIO/, "microphone"],
  [/BODY_SENSORS|OTHER_SENSORS/, "body sensors"],
  [/INTERNET/, "internet"],
];

export function readableAndroidPermissions(permissions: string[]): string {
  const labels = new Set<string>();
  for (const permission of permissions) {
    const match = ANDROID_PERMISSION_LABELS.find(([pattern]) => pattern.test(permission));
    if (match) labels.add(match[1]);
  }
  const list = [...labels];
  if (list.length === 0) return "Nothing special";
  const [first, ...rest] = list;
  const capitalized = `${first.charAt(0).toUpperCase()}${first.slice(1)}`;
  return rest.length > 0 ? `${capitalized}, ${rest.join(", ")}` : capitalized;
}
