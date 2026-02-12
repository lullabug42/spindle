<!--
  ServiceGroupDetail: Detail page for one service group. Shows card view or topology graph;
  supports opening a service via query (name@version). Starts/stops polling with the view.
  Provides service deletion with error handling.
-->
<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from "vue";
import { useRoute } from "vue-router";
import { NButton, NSpace, NFlex, useMessage, NModal, NInput, NTooltip } from "naive-ui";
import { GridViewOutlined, HubOutlined, EditOutlined } from "@vicons/material";
import { NIcon } from "naive-ui";
import ServiceItem from "@/components/services-view/ServiceItem.vue";
import TopologyGraph from "@/components/services-view/TopologyGraph.vue";
import ServiceDetailModal from "@/components/services-view/ServiceDetailModal.vue";
import { useServiceStore } from "@/stores/serviceStore";
import type { ServiceItem as ServiceItemType } from "@/types/service.types";

const route = useRoute();
const store = useServiceStore();
const message = useMessage();

const detailModalVisible = ref(false);
const selectedService = ref<ServiceItemType | null>(null);

/** Alias editing modal state. */
const aliasModalVisible = ref(false);
const aliasInput = ref("");
const isSavingAlias = ref(false);

/** Resolved group id from route; NaN when invalid (e.g. non-numeric param). */
const groupId = computed(() => {
  const id = route.params.groupId;
  const num = typeof id === "string" ? parseInt(id, 10) : Number(id);
  return Number.isFinite(num) ? num : NaN;
});

/** Current group from store, or null when id is invalid or not found. */
const group = computed(() =>
  Number.isFinite(groupId.value) ? store.groupById.get(groupId.value) ?? null : null
);

/** Query param "service" (e.g. "name@version") to open detail modal on load. */
const openServiceKey = computed(() => {
  const q = route.query.service;
  return typeof q === "string" ? q : null;
});

watch(
  openServiceKey,
  (key) => {
    if (!key || !group.value) return;
    const [name, version] = key.split("@");
    const svc = group.value.services.find((s) => s.name === name && s.version === version);
    if (svc) {
      selectedService.value = svc;
      detailModalVisible.value = true;
    }
  },
  { immediate: true }
);

onMounted(() => {
  store.startPolling();
});

onUnmounted(() => {
  store.stopPolling();
});

/** Opens the service detail modal for the given service. */
function openDetail(service: ServiceItemType) {
  selectedService.value = service;
  detailModalVisible.value = true;
}

/**
 * Handles service deletion from the detail view.
 * Shows success/error messages based on the operation result.
 * @param service - The service to delete.
 */
async function onServiceDelete(service: ServiceItemType): Promise<void> {
  try {
    await store.removeServiceFromStore({
      name: service.name,
      version: service.version,
    });
    message.success(`Service "${service.name}:${service.version}" deleted successfully`);
  } catch (error) {
    message.error(`${error}`);
  }
}

/** Opens the alias editing modal. */
function openAliasModal() {
  aliasInput.value = group.value?.alias ?? "";
  aliasModalVisible.value = true;
}

/** Closes the alias editing modal. */
function closeAliasModal() {
  aliasModalVisible.value = false;
}

/** Saves the alias (or removes if empty). */
async function saveAlias() {
  if (!group.value) return;
  isSavingAlias.value = true;
  try {
    const trimmed = aliasInput.value.trim();
    if (trimmed) {
      await store.setGroupAlias(group.value.group_id, trimmed);
      message.success(`Alias set to "${trimmed}"`);
    } else {
      await store.removeGroupAlias(group.value.group_id);
      message.success("Alias removed");
    }
    closeAliasModal();
  } catch (error) {
    message.error(`Failed to save alias: ${error}`);
  } finally {
    isSavingAlias.value = false;
  }
}
</script>

<template>
  <div class="service-group-detail">
    <n-flex v-if="group" vertical :size="16">
      <n-flex align="center" justify="space-between" class="detail-header">
        <n-flex align="center" :size="12">
          <n-tooltip trigger="hover">
            <template #trigger>
              <h2 style="margin: 0; cursor: pointer" @click="openAliasModal">{{ group.displayName }}</h2>
            </template>
            Click to edit alias
          </n-tooltip>
          <n-button quaternary circle size="small" @click="openAliasModal">
            <template #icon>
              <n-icon :component="EditOutlined" />
            </template>
          </n-button>
          <span class="group-meta">ID {{ group.group_id }} · {{ group.services.length }} services</span>
        </n-flex>
        <n-space>
          <n-button quaternary :type="store.detailViewMode === 'card' ? 'primary' : undefined"
            @click="store.detailViewMode = 'card'">
            <template #icon>
              <n-icon :component="GridViewOutlined" />
            </template>
            Card View
          </n-button>
          <n-button quaternary :type="store.detailViewMode === 'graph' ? 'primary' : undefined"
            @click="store.detailViewMode = 'graph'">
            <template #icon>
              <n-icon :component="HubOutlined" />
            </template>
            Graph View
          </n-button>
        </n-space>
      </n-flex>

      <div v-if="store.detailViewMode === 'card'" class="card-view">
        <div class="card-grid">
          <ServiceItem 
            v-for="svc in group.services" 
            :key="`${svc.name}-${svc.version}`" 
            :service="svc" 
            layout="card"
            @click="openDetail(svc)"
            @delete="onServiceDelete"
          />
        </div>
      </div>

      <div v-else class="graph-view">
        <TopologyGraph :group="group" @node-click="openDetail" />
      </div>
    </n-flex>

    <div v-else class="empty-state">
      Group not found or loading.
    </div>

    <ServiceDetailModal v-model:visible="detailModalVisible" :service="selectedService" :group="group" />

    <!-- Alias Editing Modal -->
    <n-modal
      v-model:show="aliasModalVisible"
      preset="card"
      title="Edit Group Alias"
      style="width: 400px"
      :mask-closable="false"
    >
      <n-input
        v-model:value="aliasInput"
        placeholder="Enter alias (leave empty to remove)"
        maxlength="64"
        clearable
        @keyup.enter="saveAlias"
      />
      <template #footer>
        <n-space justify="end">
          <n-button @click="closeAliasModal">Cancel</n-button>
          <n-button type="primary" :loading="isSavingAlias" @click="saveAlias">Save</n-button>
        </n-space>
      </template>
    </n-modal>
  </div>
</template>

<style scoped>
.detail-header {
  padding-bottom: 0.5rem;
  border-bottom: 1px solid var(--n-border-color);
}

.group-meta {
  color: var(--n-text-color-3);
  font-size: 0.9rem;
}

.card-view {
  width: 100%;
}

.card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 1rem;
}

.graph-view {
  width: 100%;
  min-height: 500px;
}

.empty-state {
  padding: 2rem;
  text-align: center;
  color: var(--n-text-color-3);
}
</style>
