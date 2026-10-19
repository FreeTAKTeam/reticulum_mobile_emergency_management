/** Headings shared by the Settings form pages and the matching setup wizard steps. */
export interface SettingsPageCopy {
  title: string;
  description: string;
}

export const SETTINGS_PAGE_COPY = {
  identity: { title: "Identity", description: "Who this node is to others on the mesh." },
  network: {
    title: "Network interfaces",
    description: "How this node reaches the mesh: TCP servers and LoRa radio.",
  },
  rnodePairing: {
    title: "Pair RNode device",
    description: "Find a LoRa radio over Bluetooth or USB and use it for this node.",
  },
  routing: {
    title: "Routing & hubs",
    description: "Whether messages travel peer to peer or through an RCH hub.",
  },
  location: {
    title: "Location sharing",
    description: "Share this phone's position with your peers on the map.",
  },
  sos: { title: "SOS", description: "How an emergency alert is raised and what it sends." },
} satisfies Record<string, SettingsPageCopy>;
