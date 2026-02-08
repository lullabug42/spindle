<!--
  ServicesOverview: Lists all service groups as cards (grid/list). Starts polling on mount,
  stops on unmount. Clicking a service navigates to that group's detail with service pre-selected.
-->
<script setup lang="ts">
import { onMounted, onUnmounted } from "vue";
import { useRouter } from "vue-router";
import { NButton, NSpace, NFlex } from "naive-ui";
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
  </n-flex>
</template>
