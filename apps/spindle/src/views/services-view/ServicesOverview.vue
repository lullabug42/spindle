<!--
  ServicesOverview: Lists all service groups as cards (grid/list). Starts polling on mount,
  stops on unmount. Clicking a service navigates to that group's detail with service pre-selected.
-->
<script setup lang="ts">
import { onMounted, onUnmounted } from "vue";
import { useRouter } from "vue-router";
import { NButton, NSpace, NFlex, NSwitch, NCard } from "naive-ui";
import { GridViewOutlined, ViewListOutlined } from "@vicons/material";
import { NIcon } from "naive-ui";
import ServiceGroupCard from "@/components/services-view/ServiceGroupCard.vue";
import { useServiceStore } from "@/stores/serviceStore";
import type { ServiceItem as ServiceItemType } from "@/types/service.types";

const router = useRouter();
const store = useServiceStore();

onMounted(() => {
  store.startPolling();
});

onUnmounted(() => {
  store.stopPolling();
});

/**
 * Navigates to the group detail page and opens the service in the detail modal via query.
 * @param service - The clicked service (used for group_id and name@version query).
 */
function onServiceClick(service: ServiceItemType) {
  router.push({
    name: "ServiceGroupDetail",
    params: { groupId: String(service.group_id) },
    query: { service: `${service.name}@${service.version}` },
  });
}

async function onMockSwitch(value: boolean) {
  await store.setUseMock(value);
}
</script>

<template>
  <n-flex vertical :size="16">
    <n-flex align="center" justify="space-between">
      <h2 style="margin: 0">Services</h2>
      <n-space>
        <n-button quaternary :type="store.overviewViewMode === 'grid' ? 'primary' : undefined"
          @click="store.overviewViewMode = 'grid'">
          <template #icon>
            <n-icon :component="GridViewOutlined" />
          </template>
          Grid
        </n-button>
        <n-button quaternary :type="store.overviewViewMode === 'list' ? 'primary' : undefined"
          @click="store.overviewViewMode = 'list'">
          <template #icon>
            <n-icon :component="ViewListOutlined" />
          </template>
          List
        </n-button>
      </n-space>
    </n-flex>

    <n-flex v-if="store.loading && store.groups.length === 0" justify="center">
      Loading...
    </n-flex>

    <n-flex v-else vertical :size="16">
      <ServiceGroupCard v-for="group in store.groups" :key="group.group_id" :group="group"
        :view-mode="store.overviewViewMode" @service-click="onServiceClick" />
    </n-flex>

    <n-card class="mock-toggle-card" size="small" embedded>
      <n-flex align="center" justify="space-between" class="mock-toggle-inner">
        <div class="mock-toggle-label">
          <span class="mock-toggle-title">Use Mock Data</span>
          <span class="mock-toggle-desc">
            {{ store.useMock ? "Show local mock service groups; no backend required." : "Load real data from the service manager." }}
          </span>
        </div>
        <n-switch
          :value="store.useMock"
          @update:value="onMockSwitch"
        />
      </n-flex>
    </n-card>
  </n-flex>
</template>

<style scoped>
.mock-toggle-card {
  margin-top: 8px;
  border-radius: 10px;
  border: 1px solid var(--n-border-color);
  background: var(--n-color-modal);
}

.mock-toggle-inner {
  width: 100%;
}

.mock-toggle-label {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.mock-toggle-title {
  font-weight: 600;
  font-size: 14px;
  color: var(--n-text-color);
}

.mock-toggle-desc {
  font-size: 12px;
  color: var(--n-text-color-3);
}
</style>
