export type AppIcon =
  | "action-messages"
  | "chat"
  | "checklists"
  | "dashboard"
  | "events"
  | "map"
  | "more"
  | "peers"
  | "settings";

export interface NavigationItem {
  path: string;
  label: string;
  icon: AppIcon;
}

export const footerItems: NavigationItem[] = [
  { path: "/dashboard", label: "Dashboard", icon: "dashboard" },
  { path: "/inbox", label: "Chat", icon: "chat" },
  { path: "/events", label: "Events", icon: "events" },
  { path: "/telemetry", label: "Map", icon: "map" },
];

export const menuItems: NavigationItem[] = [
  { path: "/inbox", label: "Chat", icon: "chat" },
  { path: "/messages", label: "Action Messages", icon: "action-messages" },
  { path: "/events", label: "Events", icon: "events" },
  { path: "/checklists", label: "Checklists", icon: "checklists" },
  { path: "/telemetry", label: "Map", icon: "map" },
  { path: "/peers", label: "Peers", icon: "peers" },
  { path: "/settings", label: "Settings", icon: "settings" },
];

export const iconPaths: Record<AppIcon, string[]> = {
  "action-messages": [
    "M8 4.5h6l4 4v10a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2v-12a2 2 0 0 1 2-2Z",
    "M14 4.5v4h4",
    "M9 12h6",
    "M9 15.5h6",
  ],
  chat: [
    "M6 7.5h12a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2H11l-4 3v-3H6a2 2 0 0 1-2-2v-6a2 2 0 0 1 2-2Z",
    "M8 11h8",
    "M8 14h5",
  ],
  checklists: [
    "M8 5h8a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2Z",
    "M9.5 4h5a1 1 0 0 1 1 1v1h-7V5a1 1 0 0 1 1-1Z",
    "m9.5 11 1.5 1.5 3.5-3.5",
    "M9.5 16h5",
  ],
  dashboard: [
    "M5 5h5v5H5z",
    "M14 5h5v8h-5z",
    "M5 14h5v5H5z",
    "M14 16h5v3h-5z",
  ],
  events: ["M12 4.5 19 18.5H5z", "M12 9v4", "M12 16.2h.01"],
  map: [
    "M12 20.5s5-4.7 5-9.1a5 5 0 1 0-10 0c0 4.4 5 9.1 5 9.1Z",
    "M12 13.2a1.9 1.9 0 1 0 0-3.8 1.9 1.9 0 0 0 0 3.8Z",
  ],
  more: ["M5 7h14", "M5 12h14", "M5 17h14"],
  peers: [
    "M9.5 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z",
    "M4.5 19a5 5 0 0 1 10 0",
    "M16.5 10.5a2.5 2.5 0 1 0 0-5",
    "M15.7 14.5a4.4 4.4 0 0 1 3.8 4.5",
  ],
  settings: [
    "M5 7h10",
    "M5 17h14",
    "M15 9a2 2 0 1 0 0-4 2 2 0 0 0 0 4Z",
    "M9 19a2 2 0 1 0 0-4 2 2 0 0 0 0 4Z",
  ],
};
