import type { RouteRecordRaw } from "vue-router";

/** Settings sub-pages; the app header keeps the "Settings" title, each page names itself. */
export const settingsRoutes: RouteRecordRaw[] = [
  {
    path: "/settings/identity",
    name: "settings-identity",
    meta: { title: "Settings" },
    component: () => import("./views/settings/SettingsIdentityView.vue"),
  },
  {
    path: "/settings/network",
    component: () => import("./views/settings/SettingsNetworkView.vue"),
    children: [
      {
        path: "",
        name: "settings-network",
        meta: { title: "Settings" },
        component: () => import("./views/settings/SettingsNetworkFormView.vue"),
      },
      {
        path: "rnode-pairing",
        name: "settings-rnode-pairing",
        meta: { title: "Settings" },
        component: () => import("./views/settings/SettingsRnodePairingView.vue"),
      },
    ],
  },
  {
    path: "/settings/routing",
    name: "settings-routing",
    meta: { title: "Settings" },
    component: () => import("./views/settings/SettingsRoutingView.vue"),
  },
  {
    path: "/settings/location",
    name: "settings-location",
    meta: { title: "Settings" },
    component: () => import("./views/settings/SettingsLocationView.vue"),
  },
  {
    path: "/settings/map",
    name: "settings-map",
    meta: { title: "Settings" },
    component: () => import("./views/settings/SettingsMapView.vue"),
  },
  {
    path: "/settings/sos",
    name: "settings-sos",
    meta: { title: "Settings" },
    component: () => import("./views/settings/SettingsSosView.vue"),
  },
  {
    path: "/settings/power",
    name: "settings-power",
    meta: { title: "Settings" },
    component: () => import("./views/settings/SettingsPowerView.vue"),
  },
  {
    path: "/settings/plugins",
    name: "settings-plugins",
    meta: { title: "Settings" },
    component: () => import("./views/settings/SettingsPluginsView.vue"),
  },
  {
    path: "/settings/plugins/:pluginId",
    name: "settings-plugin-detail",
    meta: { title: "Settings" },
    props: true,
    component: () => import("./views/settings/SettingsPluginDetailView.vue"),
  },
  {
    path: "/settings/share-setup",
    name: "settings-share-setup",
    meta: { title: "Settings" },
    component: () => import("./views/settings/SettingsShareSetupView.vue"),
  },
  {
    path: "/settings/node",
    name: "settings-node",
    meta: { title: "Settings" },
    component: () => import("./views/settings/SettingsNodeView.vue"),
  },
  {
    path: "/settings/about",
    name: "settings-about",
    meta: { title: "Settings" },
    component: () => import("./views/settings/SettingsAboutView.vue"),
  },
];
