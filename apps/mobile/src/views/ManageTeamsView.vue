<script setup lang="ts">
import { computed, onMounted, ref, useTemplateRef, watch } from "vue";
import {
  PhArrowLeft as ArrowLeft,
  PhCaretRight as CaretRight,
  PhLockSimple as LockSimple,
  PhPlus as Plus,
  PhQrCode as QrCode,
  PhShareNetwork as ShareNetwork,
  PhX as X,
} from "@phosphor-icons/vue";
import { YELLOW_TEAM_UID } from "@reticulum/node-client";
import { useRoute, useRouter } from "vue-router";

import TeamQrExchange from "../components/TeamQrExchange.vue";
import {
  teamColorHex,
  teamColorLabel,
  useTeamDirectory,
} from "../composables/useTeamDirectory";
import { notifyOperationalUpdate } from "../services/notifications";
import { copyToClipboard, shareText } from "../services/peerExchange";
import { useNodeStore } from "../stores/nodeStore";

interface TeamQrExchangeHandle {
  scanQrCode: () => Promise<void>;
  showQrCode: (teamUid?: string) => Promise<void>;
}

const route = useRoute();
const router = useRouter();
const nodeStore = useNodeStore();
const qrExchange = useTemplateRef<TeamQrExchangeHandle>("qrExchange");
const {
  activeTeamUid,
  addablePeers,
  addLocalMember,
  availableLocalTeams,
  createLocalTeam,
  deleteLocalTeam,
  exportLocalTeamText,
  importLocalTeamPayload,
  localTeams,
  removeLocalMember,
  saveTeamAlias,
  teamSections,
} = useTeamDirectory();

const newTeamOpen = ref(false);
const newTeamUid = ref("");
const newTeamAlias = ref("");
const importDraft = ref("");
const selectedTeamUid = ref("");
const aliasDraft = ref("");
const addMemberDraft = ref("");
const feedback = ref("");
const busy = ref(false);

const myTeams = computed(() => teamSections.value.filter((section) => section.local));
const rchOnlyTeams = computed(() => teamSections.value.filter((section) => !section.local && section.rch));
const selectedSection = computed(() => (
  teamSections.value.find((section) => section.team.uid === selectedTeamUid.value)
));
const qrTeams = computed(() => localTeams.value.map((team) => {
  const section = teamSections.value.find((candidate) => candidate.team.uid === team.teamUid);
  return {
    teamUid: team.teamUid,
    label: section?.label || teamColorLabel(section?.team.color || "Team"),
    memberDestinations: [...team.memberDestinations],
  };
}));

watch(selectedSection, (section) => {
  aliasDraft.value = section?.label ?? "";
  addMemberDraft.value = "";
});

onMounted(() => {
  if (route.query.action === "add") newTeamOpen.value = true;
});

function sourceLabel(section: (typeof teamSections.value)[number]): string {
  if (section.local && section.rch) return "LOCAL + RCH";
  return section.local ? "LOCAL" : "RCH";
}

function memberSource(localMember: boolean, hasRchMember: boolean): string {
  if (localMember && hasRchMember) return "LOCAL + RCH";
  return localMember ? "LOCAL" : "RCH";
}

async function runAction(
  action: () => Promise<void>,
  successMessage: string,
  failureAction: string,
): Promise<boolean> {
  if (busy.value) return false;
  busy.value = true;
  feedback.value = "";
  try {
    await action();
    feedback.value = successMessage;
    await notifyOperationalUpdate("Teams", successMessage, { route: "/settings/teams" });
    return true;
  } catch (error: unknown) {
    const detail = error instanceof Error ? error.message : String(error);
    feedback.value = `${failureAction} failed - ${detail}`;
    nodeStore.setLastError(detail);
    return false;
  } finally {
    busy.value = false;
  }
}

async function goBack(): Promise<void> {
  await router.push(route.query.from === "peers" ? "/peers" : "/settings");
}

function openTeam(teamUid: string): void {
  selectedTeamUid.value = teamUid;
}

function closeTeam(): void {
  selectedTeamUid.value = "";
}

async function createTeam(): Promise<void> {
  const teamUid = newTeamUid.value;
  const created = await runAction(
    () => createLocalTeam(teamUid, newTeamAlias.value),
    `${newTeamAlias.value.trim() || "Local team"} created.`,
    "create team",
  );
  if (!created) return;
  newTeamOpen.value = false;
  newTeamUid.value = "";
  newTeamAlias.value = "";
  openTeam(teamUid);
}

async function saveAlias(): Promise<void> {
  const section = selectedSection.value;
  if (!section) return;
  const alias = await saveTeamAlias(section.team.uid, aliasDraft.value);
  aliasDraft.value = alias || teamColorLabel(section.team.color);
  feedback.value = alias ? "Local alias saved." : "Local alias cleared.";
}

async function addMember(): Promise<void> {
  const section = selectedSection.value;
  if (!section) return;
  const added = await runAction(
    () => addLocalMember(section.team.uid, addMemberDraft.value),
    "Saved peer added to the team.",
    "add team member",
  );
  if (added) addMemberDraft.value = "";
}

async function removeMember(destination: string): Promise<void> {
  const section = selectedSection.value;
  if (!section) return;
  await runAction(
    () => removeLocalMember(section.team.uid, destination),
    "Peer removed from the local team.",
    "remove team member",
  );
}

async function removeTeam(): Promise<void> {
  const section = selectedSection.value;
  if (!section || section.team.uid === YELLOW_TEAM_UID) return;
  if (!window.confirm(`Delete the local ${section.label} team?`)) return;
  const removed = await runAction(
    () => deleteLocalTeam(section.team.uid),
    "Local team removed.",
    "remove team",
  );
  if (removed) closeTeam();
}

async function exportTeam(teamUid: string): Promise<void> {
  await runAction(async () => {
    const payload = exportLocalTeamText(teamUid);
    const section = teamSections.value.find((candidate) => candidate.team.uid === teamUid);
    const [copied, shared] = await Promise.allSettled([
      copyToClipboard(payload),
      shareText(`${section?.label || "REM"} team`, payload),
    ]);
    if (copied.status === "rejected" && shared.status === "rejected") {
      throw new AggregateError(
        [copied.reason, shared.reason],
        "Team export and clipboard copy failed.",
      );
    }
  }, "Local team exported.", "export team");
}

async function showTeamQr(teamUid: string): Promise<void> {
  await qrExchange.value?.showQrCode(teamUid);
}

async function scanTeamQr(): Promise<void> {
  await qrExchange.value?.scanQrCode();
}

async function handleQrImport(payload: string): Promise<void> {
  const importedTeamUid = await importLocalTeamPayload(payload);
  feedback.value = "Team imported and merged by color.";
  openTeam(importedTeamUid);
}

async function importTeamJson(): Promise<void> {
  let importedTeamUid = "";
  const imported = await runAction(async () => {
    importedTeamUid = await importLocalTeamPayload(importDraft.value);
  }, "Team imported and merged by color.", "import team");
  if (!imported) return;
  importDraft.value = "";
  newTeamOpen.value = false;
  openTeam(importedTeamUid);
}
</script>

<template>
  <section class="manage-teams-view">
    <header class="manage-intro">
      <button type="button" class="back-button" @click="goBack">
        <ArrowLeft :size="22" aria-hidden="true" />
        <span>{{ route.query.from === "peers" ? "Back to peers" : "Back to settings" }}</span>
      </button>
      <p>Local teams can be edited and shared. Hub membership is read-only.</p>
    </header>

    <div class="primary-actions">
      <button type="button" class="primary-button" :disabled="busy || availableLocalTeams.length === 0" @click="newTeamOpen = true">
        <Plus :size="24" aria-hidden="true" />
        Add team
      </button>
      <button type="button" :disabled="busy" @click="scanTeamQr">
        <QrCode :size="24" aria-hidden="true" />
        Scan QR
      </button>
    </div>

    <p v-if="feedback" class="feedback" role="status">{{ feedback }}</p>

    <section class="team-directory" aria-labelledby="my-teams-heading">
      <h2 id="my-teams-heading">My teams</h2>
      <div class="directory-list">
        <article v-for="section in myTeams" :key="section.team.uid" class="directory-row">
          <button type="button" class="team-row-main" @click="openTeam(section.team.uid)">
            <span
              class="team-color-dot"
              :style="{ '--team-color': teamColorHex(section.team.color) }"
              aria-hidden="true"
            ></span>
            <span class="team-row-copy">
              <strong>{{ section.label }}</strong>
              <small>
                <span :style="{ color: teamColorHex(section.team.color) }">{{ teamColorLabel(section.team.color).toUpperCase() }}</span>
                <span aria-hidden="true">·</span>
                {{ section.total }} member{{ section.total === 1 ? "" : "s" }}
              </small>
            </span>
            <span class="team-row-source">
              <span v-if="section.active" class="active-badge">Active</span>
              <small>{{ sourceLabel(section) }}</small>
            </span>
            <CaretRight :size="22" aria-hidden="true" />
          </button>
          <button
            type="button"
            class="qr-row-action"
            :aria-label="`Show ${section.label} team QR code`"
            @click="showTeamQr(section.team.uid)"
          >
            <QrCode :size="24" aria-hidden="true" />
          </button>
        </article>
      </div>
    </section>

    <section v-if="rchOnlyTeams.length" class="team-directory" aria-labelledby="rch-teams-heading">
      <h2 id="rch-teams-heading">From RCH</h2>
      <div class="directory-list">
        <article v-for="section in rchOnlyTeams" :key="section.team.uid" class="directory-row">
          <button type="button" class="team-row-main" @click="openTeam(section.team.uid)">
            <span
              class="team-color-dot"
              :style="{ '--team-color': teamColorHex(section.team.color) }"
              aria-hidden="true"
            ></span>
            <span class="team-row-copy">
              <strong>{{ section.label }}</strong>
              <small>
                <span :style="{ color: teamColorHex(section.team.color) }">{{ teamColorLabel(section.team.color).toUpperCase() }}</span>
                <span aria-hidden="true">·</span>
                {{ section.total }} member{{ section.total === 1 ? "" : "s" }}
              </small>
            </span>
            <span class="read-only-source">
              <LockSimple :size="17" aria-hidden="true" />
              Read only
            </span>
            <CaretRight :size="22" aria-hidden="true" />
          </button>
        </article>
      </div>
    </section>

    <p class="hub-note"><LockSimple :size="17" aria-hidden="true" /> RCH membership is managed by the hub.</p>

    <TeamQrExchange
      ref="qrExchange"
      :teams="qrTeams"
      :show-launcher="false"
      @error="feedback = $event"
      @import="handleQrImport"
    />

    <div v-if="newTeamOpen" class="team-modal-overlay" role="dialog" aria-modal="true" aria-labelledby="new-team-title">
      <form class="team-modal" @submit.prevent="createTeam">
        <header>
          <div>
            <h2 id="new-team-title">Add local team</h2>
            <p>Choose one of the canonical team colors.</p>
          </div>
          <button type="button" class="icon-button" aria-label="Close add team" @click="newTeamOpen = false">
            <X :size="22" aria-hidden="true" />
          </button>
        </header>
        <label>
          <span>New local team color</span>
          <select v-model="newTeamUid" required>
            <option value="" disabled>Select color</option>
            <option v-for="team in availableLocalTeams" :key="team.uid" :value="team.uid">
              {{ teamColorLabel(team.color) }}
            </option>
          </select>
        </label>
        <label>
          <span>Local name</span>
          <input v-model="newTeamAlias" maxlength="48" placeholder="Friends, family, field unit…" />
        </label>
        <div class="modal-actions">
          <button type="button" @click="newTeamOpen = false">Cancel</button>
          <button type="submit" class="primary-button" :disabled="busy || !newTeamUid">Create team</button>
        </div>
        <details class="legacy-import">
          <summary>Import team JSON</summary>
          <label>
            <span>Shared team payload</span>
            <textarea v-model="importDraft" rows="4" placeholder="Paste a REM local-team export…"></textarea>
          </label>
          <button type="button" :disabled="busy || !importDraft.trim()" @click="importTeamJson">Import team</button>
        </details>
      </form>
    </div>

    <div v-if="selectedSection" class="team-modal-overlay" role="dialog" aria-modal="true" :aria-label="`Manage ${selectedSection.label}`">
      <section class="team-modal team-detail-modal">
        <header>
          <div class="detail-title">
            <span
              class="team-color-dot"
              :style="{ '--team-color': teamColorHex(selectedSection.team.color) }"
              aria-hidden="true"
            ></span>
            <div>
              <h2>{{ selectedSection.label }}</h2>
              <p>{{ teamColorLabel(selectedSection.team.color).toUpperCase() }} · {{ sourceLabel(selectedSection) }}</p>
            </div>
          </div>
          <button type="button" class="icon-button" aria-label="Close team details" @click="closeTeam">
            <X :size="22" aria-hidden="true" />
          </button>
        </header>

        <label>
          <span>Local alias</span>
          <div class="inline-field">
            <input v-model="aliasDraft" maxlength="48" :placeholder="teamColorLabel(selectedSection.team.color)" />
            <button type="button" :disabled="busy" @click="saveAlias">Save</button>
          </div>
          <small>Alias stays on this device.</small>
        </label>

        <div v-if="selectedSection.local" class="detail-actions">
          <button type="button" @click="showTeamQr(selectedSection.team.uid)">
            <QrCode :size="20" aria-hidden="true" /> Share QR
          </button>
          <button type="button" @click="exportTeam(selectedSection.team.uid)">
            <ShareNetwork :size="20" aria-hidden="true" /> Export
          </button>
        </div>

        <section class="member-section">
          <h3>Members <span>{{ selectedSection.total }}</span></h3>
          <form v-if="selectedSection.local && addablePeers(selectedSection.team.uid).length" class="add-member-form" @submit.prevent="addMember">
            <select v-model="addMemberDraft" :aria-label="`Add saved peer to ${selectedSection.label}`" required>
              <option value="" disabled>Add a saved peer…</option>
              <option v-for="peer in addablePeers(selectedSection.team.uid)" :key="peer.destination" :value="peer.destination">
                {{ peer.label || peer.displayName || peer.destination }}
              </option>
            </select>
            <button type="submit" :disabled="busy">Add</button>
          </form>
          <div class="member-list">
            <article v-for="peer in selectedSection.rows" :key="peer.destination">
              <div>
                <strong>{{ peer.displayName }}</strong>
                <small>{{ memberSource(peer.localMember, Boolean(peer.member)) }} · {{ peer.destination }}</small>
              </div>
              <button
                v-if="peer.localMember"
                type="button"
                class="remove-button"
                :disabled="busy"
                @click="removeMember(peer.destination)"
              >
                Remove
              </button>
              <span v-else class="read-only-source"><LockSimple :size="15" aria-hidden="true" /> Read only</span>
            </article>
            <p v-if="selectedSection.rows.length === 0" class="empty-copy">No members in this team.</p>
          </div>
        </section>

        <button
          v-if="selectedSection.local && selectedSection.team.uid !== YELLOW_TEAM_UID"
          type="button"
          class="delete-team-button"
          :disabled="busy"
          @click="removeTeam"
        >
          Delete local team
        </button>
      </section>
    </div>
  </section>
</template>

<style scoped src="./ManageTeamsView.css"></style>
