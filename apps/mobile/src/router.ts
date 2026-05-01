import { createRouter, createWebHistory } from "vue-router";
import ActionMessagesView from "./views/ActionMessagesView.vue";
import ChecklistView from "./views/ChecklistView.vue";
import ChecklistDetailView from "./views/ChecklistDetailView.vue";
import DashboardView from "./views/DashboardView.vue";
import EventsView from "./views/EventsView.vue";
import InboxView from "./views/InboxView.vue";
import MessageStatusHelpView from "./views/MessageStatusHelpView.vue";
import PeersDiscoveryView from "./views/PeersDiscoveryView.vue";
import SettingsView from "./views/SettingsView.vue";
import SetupWizardView from "./views/SetupWizardView.vue";
import TelemetryMapView from "./views/TelemetryMapView.vue";

const routes = [
  {
    path: "/",
    redirect: "/dashboard",
  },
  {
    path: "/messages",
    name: "messages",
    component: ActionMessagesView,
  },
  {
    path: "/inbox",
    name: "inbox",
    component: InboxView,
  },
  {
    path: "/checklists",
    name: "checklists",
    component: ChecklistView,
  },
  {
    path: "/checklists/:checklistId",
    name: "checklist-detail",
    component: ChecklistDetailView,
  },
  {
    path: "/messages/help",
    name: "message-status-help",
    component: MessageStatusHelpView,
  },
  {
    path: "/events",
    name: "events",
    component: EventsView,
  },
  {
    path: "/dashboard",
    name: "dashboard",
    component: DashboardView,
  },
  {
    path: "/settings",
    name: "settings",
    component: SettingsView,
  },
  {
    path: "/setup",
    name: "setup",
    component: SetupWizardView,
  },
  {
    path: "/peers",
    name: "peers",
    component: PeersDiscoveryView,
  },
  {
    path: "/telemetry",
    name: "telemetry",
    component: TelemetryMapView,
  },
];

export const router = createRouter({
  history: createWebHistory(),
  routes,
});
