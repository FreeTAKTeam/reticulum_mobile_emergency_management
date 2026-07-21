<script setup lang="ts">
import { computed } from "vue";
import {
  PhCaretRight as CaretRight,
  PhUsersThree as UsersThree,
} from "@phosphor-icons/vue";
import { useRouter } from "vue-router";

import { useTeamDirectory } from "../../composables/useTeamDirectory";

const router = useRouter();
const { teamSections } = useTeamDirectory();
const summary = computed(() => {
  const local = teamSections.value.filter((section) => section.local).length;
  const rchOnly = teamSections.value.filter((section) => !section.local && section.rch).length;
  return `${local} local · ${rchOnly} from RCH`;
});

async function openManageTeams(): Promise<void> {
  await router.push({ name: "manage-teams" });
}
</script>

<template>
  <section class="panel settings-team-entry" aria-labelledby="manage-teams-settings-heading">
    <div class="settings-team-copy">
      <UsersThree :size="24" aria-hidden="true" />
      <div>
        <h2 id="manage-teams-settings-heading">Manage Teams</h2>
        <p>{{ summary }}</p>
      </div>
    </div>
    <button type="button" @click="openManageTeams">
      Open
      <CaretRight :size="18" aria-hidden="true" />
    </button>
  </section>
</template>

<style scoped>
.settings-team-entry {
  align-items: center;
  display: flex;
  gap: 1rem;
  justify-content: space-between;
}

.settings-team-copy {
  align-items: center;
  color: #65d2ff;
  display: flex;
  gap: 0.8rem;
  min-width: 0;
}

.settings-team-copy h2 {
  color: #dcefff;
  font-family: var(--font-headline);
  margin: 0;
}

.settings-team-copy p {
  color: #8fa9d1;
  font-family: var(--font-body);
  margin: 0.18rem 0 0;
}

.settings-team-entry button {
  align-items: center;
  display: inline-flex;
  flex: 0 0 auto;
  gap: 0.35rem;
  min-height: 2.75rem;
}
</style>
