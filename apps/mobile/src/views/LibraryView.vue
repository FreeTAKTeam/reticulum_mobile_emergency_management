<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import {
  PhFileText as FileText,
  PhImage as ImageIcon,
  PhMagnifyingGlass as MagnifyingGlass,
  PhPlus as Plus,
} from "@phosphor-icons/vue";
import type { LibraryItem } from "@reticulum/node-client";

import { type LibraryKindFilter, type LibrarySection, useLibraryStore } from "../stores/libraryStore";
import { useNodeStore } from "../stores/nodeStore";

const library = useLibraryStore();
const nodeStore = useNodeStore();
const sharingItem = ref("");
const shareTarget = ref("");
const savedPeers = computed(() =>
  Object.values(nodeStore.savedByDestination).map((peer) => ({
    destination: peer.destination,
    label: peer.label?.trim() || peer.destination,
  })),
);

function startShare(item: LibraryItem): void {
  sharingItem.value = sharingItem.value === item.item_id ? "" : item.item_id;
  shareTarget.value = savedPeers.value[0]?.destination ?? "";
}

function confirmShare(): void {
  if (sharingItem.value && shareTarget.value) {
    void library.share(sharingItem.value, shareTarget.value);
    sharingItem.value = "";
  }
}

const sections: { id: LibrarySection; label: string }[] = [
  { id: "device", label: "On Device" },
  { id: "rch", label: "RCH" },
  { id: "shared", label: "Shared With Me" },
];
const kinds: { id: LibraryKindFilter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "image", label: "Images" },
  { id: "file", label: "Files" },
];

const emptyCopy = computed(() => {
  if (library.section === "rch") {
    if (library.hubStatus && library.hubStatus !== "ok") {
      return `The hub did not return a Library (${library.hubStatus}). It may run an older RCH.`;
    }
    return "No hub files listed. Use Refresh to ask the configured RCH.";
  }
  if (library.section === "shared") {
    return "Files other REM operators share with you appear here.";
  }
  return "No files on this device yet. Import a photo or document to keep it available offline.";
});
const pageStart = computed(() => (library.page - 1) * 20);

function size(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function availability(item: LibraryItem): string {
  switch (item.availability) {
    case "remote": return "Not downloaded";
    case "downloading": return "Downloading";
    case "failed": return "Missing on device";
    default: return "On device";
  }
}

function editMissions(item: LibraryItem): void {
  const answer = window.prompt("Mission UIDs, separated by commas", item.mission_uids.join(", "));
  if (answer !== null) {
    void library.setMissions(item.item_id, answer.split(",").map((uid) => uid.trim()).filter(Boolean));
  }
}

function remove(item: LibraryItem): void {
  if (window.confirm(`Remove "${item.name}" from this device? Copies shared with others are not affected.`)) {
    void library.remove(item.item_id);
  }
}

function changePage(delta: number): void {
  library.page = Math.max(1, library.page + delta);
  void library.refresh();
}

watch(() => [library.section, library.kind, library.mission, library.search], () => {
  library.page = 1;
  void library.refresh();
});

onMounted(() => {
  void library.init();
});
</script>

<template>
  <section class="view">
    <nav class="library-tabs" aria-label="Library source">
      <button
        v-for="entry in sections"
        :key="entry.id"
        type="button"
        class="tab-button"
        :class="{ active: library.section === entry.id }"
        @click="library.section = entry.id"
      >
        {{ entry.label }}
      </button>
    </nav>

    <section class="panel">
      <div class="filter-row">
        <div class="kind-chips" role="radiogroup" aria-label="Type">
          <button
            v-for="entry in kinds"
            :key="entry.id"
            type="button"
            role="radio"
            class="chip"
            :aria-checked="library.kind === entry.id"
            :class="{ active: library.kind === entry.id }"
            @click="library.kind = entry.id"
          >
            {{ entry.label }}
          </button>
        </div>
        <select v-model="library.mission" class="mission-filter" aria-label="Mission">
          <option value="">All missions</option>
          <option v-for="uid in library.missions" :key="uid" :value="uid">{{ uid }}</option>
        </select>
      </div>
      <label class="panel-search">
        <MagnifyingGlass :size="21" aria-hidden="true" />
        <input v-model="library.search" type="search" placeholder="Search by name" aria-label="Search the library" />
      </label>
      <div v-if="library.section === 'device'" class="import-row">
        <button type="button" :disabled="library.busy" @click="library.importFile('image')">
          <Plus :size="18" aria-hidden="true" /> Photo
        </button>
        <button type="button" :disabled="library.busy" @click="library.importFile('all')">
          <Plus :size="18" aria-hidden="true" /> Document
        </button>
      </div>
      <div v-if="library.section === 'rch'" class="import-row">
        <button type="button" :disabled="library.busy" @click="library.refreshHub()">Refresh from hub</button>
      </div>
      <p v-if="library.lastError" class="error-copy" role="alert">{{ library.lastError }}</p>

      <div v-if="library.items.length > 0" class="library-list">
        <article v-for="item in library.items" :key="item.item_id" class="library-item">
          <div class="preview" aria-hidden="true">
            <img v-if="library.thumbnails[item.item_id]" :src="library.thumbnails[item.item_id]" alt="" />
            <ImageIcon v-else-if="item.kind === 'image'" :size="30" />
            <FileText v-else :size="30" />
          </div>
          <div class="item-copy">
            <p class="item-name">{{ item.name }}</p>
            <p class="item-meta">{{ size(item.size) }} | {{ item.media_type || item.kind }} | {{ availability(item) }}</p>
            <p v-if="item.mission_uids.length > 0" class="item-missions">{{ item.mission_uids.join(", ") }}</p>
          </div>
          <div class="actions">
            <button v-if="item.availability === 'local'" type="button" @click="library.open(item.item_id)">Open</button>
            <button
              v-if="item.source !== 'local' && item.availability !== 'local'"
              type="button"
              :disabled="item.availability === 'downloading'"
              @click="library.download(item.item_id)"
            >
              {{ item.availability === "downloading" ? "Downloading" : item.availability === "failed" ? "Retry" : "Download" }}
            </button>
            <button v-if="item.availability === 'local'" type="button" @click="startShare(item)">Share</button>
            <button v-if="item.source === 'local'" type="button" @click="editMissions(item)">Missions</button>
            <button
              v-if="item.availability === 'local' || item.source === 'peer'"
              type="button"
              class="danger"
              @click="remove(item)"
            >
              Remove
            </button>
          </div>
          <div v-if="sharingItem === item.item_id" class="share-row">
            <select v-model="shareTarget" class="mission-filter" aria-label="Share with">
              <option v-for="peer in savedPeers" :key="peer.destination" :value="peer.destination">{{ peer.label }}</option>
            </select>
            <button type="button" :disabled="!shareTarget" @click="confirmShare">Send</button>
            <p v-if="savedPeers.length === 0" class="empty-copy">Save a peer first to share with it.</p>
          </div>
        </article>
        <nav v-if="library.total > 20" class="pager" aria-label="Library pages">
          <button type="button" :disabled="library.page <= 1" @click="changePage(-1)">Previous</button>
          <span>{{ pageStart + 1 }}-{{ pageStart + library.items.length }} of {{ library.total }}</span>
          <button type="button" :disabled="!library.hasNext" @click="changePage(1)">Next</button>
        </nav>
      </div>
      <p v-else-if="!library.loading" class="empty-copy">{{ emptyCopy }}</p>
    </section>
  </section>
</template>

<style scoped src="./LibraryView.css"></style>
