import { createRouter, createWebHistory } from "vue-router";

const routes = [
  {
    path: "/",
    redirect: "/messages",
  },
  {
    path: "/messages",
    name: "messages",
    component: () => import("./views/ActionMessagesView.vue"),
  },
  {
    path: "/events",
    name: "events",
    component: () => import("./views/EventsView.vue"),
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
    path: "/peers",
    name: "peers",
    component: () => import("./views/PeersDiscoveryView.vue"),
  },
];

export const router = createRouter({
  history: createWebHistory(),
  routes,
});
