<script setup lang="ts">
import { computed } from "vue";
import type {
  InstalledPluginRecord,
  PluginPermissionsRecord,
} from "@reticulum/node-client";

const props = defineProps<{
  plugin: InstalledPluginRecord;
  pending: boolean;
}>();

const emit = defineEmits<{
  setEnabled: [pluginId: string, enabled: boolean];
  grantPermissions: [pluginId: string, permissions: PluginPermissionsRecord];
}>();

type PermissionKey = keyof PluginPermissionsRecord;

interface PermissionDefinition {
  key: PermissionKey;
  label: string;
}

const permissionDefinitions: PermissionDefinition[] = [
  { key: "storagePlugin", label: "Plug-in storage" },
  { key: "storageShared", label: "Shared storage" },
  { key: "messagesRead", label: "Read messages" },
  { key: "messagesWrite", label: "Write messages" },
  { key: "lxmfSend", label: "Send LXMF" },
  { key: "lxmfReceive", label: "Receive LXMF" },
  { key: "notificationsRaise", label: "Raise notifications" },
];

const declaredPermissions = computed(() =>
  permissionDefinitions.filter((permission) => props.plugin.permissions[permission.key]),
);

const grantedCount = computed(
  () =>
    declaredPermissions.value.filter(
      (permission) => props.plugin.grantedPermissions[permission.key],
    ).length,
);

const pluginSummary = computed(() => {
  if (declaredPermissions.value.length === 0) {
    return "No host permissions declared";
  }
  return `${grantedCount.value}/${declaredPermissions.value.length} permissions granted`;
});

function updatePermission(permission: PermissionDefinition, granted: boolean): void {
  emit("grantPermissions", props.plugin.id, {
    ...props.plugin.grantedPermissions,
    [permission.key]: granted,
  });
}
</script>

<template>
  <article class="plugin-management-card">
    <header class="plugin-management-header">
      <div class="plugin-management-title">
        <h3>{{ plugin.name }}</h3>
        <p>{{ plugin.id }} | {{ plugin.version }} | {{ plugin.libraryPath }}</p>
      </div>
      <span class="plugin-state">{{ plugin.state }}</span>
    </header>

    <div class="plugin-management-meta">
      <span>{{ pluginSummary }}</span>
      <span>{{ plugin.messages.length }} LXMF message type{{ plugin.messages.length === 1 ? "" : "s" }}</span>
    </div>

    <div class="plugin-management-actions">
      <button
        type="button"
        :disabled="pending"
        @click="emit('setEnabled', plugin.id, plugin.state === 'Disabled')"
      >
        {{ plugin.state === "Disabled" ? "Enable Plug-in" : "Disable Plug-in" }}
      </button>
    </div>

    <div v-if="declaredPermissions.length > 0" class="plugin-permissions-grid">
      <label
        v-for="permission in declaredPermissions"
        :key="permission.key"
        class="plugin-permission"
      >
        <span>{{ permission.label }}</span>
        <input
          type="checkbox"
          :checked="plugin.grantedPermissions[permission.key]"
          :disabled="pending"
          @change="updatePermission(permission, ($event.target as HTMLInputElement).checked)"
        />
      </label>
    </div>

    <p v-else class="plugin-description">This plug-in did not declare host permissions.</p>
  </article>
</template>

<style scoped>
.plugin-management-card {
  background: rgb(7 20 44 / 76%);
  border: 1px solid rgb(67 106 165 / 35%);
  border-radius: 12px;
  display: grid;
  gap: 0.62rem;
  padding: 0.72rem;
}

.plugin-management-header {
  align-items: start;
  display: flex;
  gap: 0.7rem;
  justify-content: space-between;
}

.plugin-management-title {
  display: grid;
  gap: 0.12rem;
  min-width: 0;
}

.plugin-management-title h3,
.plugin-management-title p,
.plugin-description {
  margin: 0;
}

.plugin-management-title h3 {
  color: #d5eaff;
  font-family: var(--font-headline);
  font-size: 1rem;
}

.plugin-management-title p,
.plugin-description,
.plugin-management-meta {
  color: #8fa9d1;
  font-family: var(--font-body);
  overflow-wrap: anywhere;
}

.plugin-management-title p,
.plugin-management-meta {
  font-size: 0.78rem;
}

.plugin-state {
  background: rgb(13 120 195 / 34%);
  border: 1px solid rgb(95 193 255 / 42%);
  border-radius: 999px;
  color: #8fe3ff;
  flex: 0 0 auto;
  font-family: var(--font-ui);
  font-size: 0.68rem;
  letter-spacing: 0.08em;
  padding: 0.24rem 0.5rem;
  text-transform: uppercase;
}

.plugin-management-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem 1rem;
}

.plugin-management-actions {
  display: flex;
  justify-content: flex-end;
}

.plugin-permissions-grid {
  display: grid;
  gap: 0.5rem;
  grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
}

.plugin-permission {
  align-items: center;
  background: rgb(6 17 38 / 68%);
  border: 1px solid rgb(70 110 174 / 30%);
  border-radius: 10px;
  color: #a0b7db;
  display: grid;
  font-family: var(--font-body);
  font-size: 0.82rem;
  gap: 0.5rem;
  grid-template-columns: 1fr auto;
  padding: 0.44rem 0.52rem;
}
</style>
