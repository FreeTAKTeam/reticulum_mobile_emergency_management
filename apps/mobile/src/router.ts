import { createRouter, createWebHistory } from "vue-router";

const routes = [
  {
    path: "/",
    redirect: "/dashboard",
  },
  {
    path: "/messages",
    name: "messages",
    component: () => import("./views/ActionMessagesView.vue"),
  },
  {
    path: "/inbox",
    name: "inbox",
    component: () => import("./views/InboxView.vue"),
  },
  {
    path: "/checklists",
    name: "checklists",
    component: () => import("./views/ChecklistView.vue"),
  },
  {
    path: "/checklists/:checklistId",
    name: "checklist-detail",
    component: () => import("./views/ChecklistDetailView.vue"),
  },
  {
    path: "/messages/help",
    name: "message-status-help",
    component: () => import("./views/MessageStatusHelpView.vue"),
  },
  {
    path: "/events",
    name: "events",
    component: () => import("./views/EventsView.vue"),
  },
  {
    path: "/events/help",
    name: "event-mecp-help",
    component: () => import("./views/EventMecpHelpView.vue"),
  },
  {
    path: "/dashboard",
    name: "dashboard",
    component: () => import("./views/DashboardView.vue"),
  },
  {
    path: "/settings",
    name: "settings",
    component: () => import("./views/SettingsView.vue"),
  },
  {
    path: "/setup",
    name: "setup",
    component: () => import("./views/SetupWizardView.vue"),
  },
  {
    path: "/peers",
    name: "peers",
    component: () => import("./views/PeersDiscoveryView.vue"),
  },
  {
    path: "/telemetry",
    name: "telemetry",
    component: () => import("./views/TelemetryMapView.vue"),
  },
];

export const router = createRouter({
  history: createWebHistory(),
  routes,
});
