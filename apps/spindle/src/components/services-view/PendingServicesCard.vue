<!--
  PendingServicesCard: Displays services that have been added but not yet reloaded.
  Shown only when there are pending services.
  In mock mode: services will be discarded on reload.
  In real mode: services will be properly grouped after reload.
-->
<script setup lang="ts">
import { NCard, NTag, NIcon, useMessage } from "naive-ui";
import { HourglassTopFilled } from "@vicons/material";
import { computed } from "vue";
import ServiceItem from "./ServiceItem.vue";
import { useServiceStore } from "@/stores/serviceStore";
import type { ServiceItem as ServiceItemType } from "@/types/service.types";

const props = defineProps<{
  /** List of pending services waiting to be reloaded. */
  services: ServiceItemType[];
  /** Whether running in mock mode. */
  isMock: boolean;
}>();

const store = useServiceStore();
const message = useMessage();

const hasServices = computed(() => props.services.length > 0);

/**
 * Handles service deletion from pending services.
 * Works in both mock and real mode.
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
    message.error(`Failed to delete service: ${error}`);
  }
}
</script>

<template>
  <n-card v-if="hasServices" class="pending-services-card" size="small">
    <template #header>
      <div class="card-header">
        <div class="header-title">
          <n-icon :component="HourglassTopFilled" size="18" class="header-icon" />
          <span>Pending Services</span>
          <n-tag size="small" type="warning" round>{{ services.length }}</n-tag>
        </div>
        <span class="header-hint">{{ isMock ? "Will be discarded on reload" : "Click Reload to apply" }}</span>
      </div>
    </template>

    <div class="services-grid">
      <ServiceItem
        v-for="service in services"
        :key="`${service.name}-${service.version}`"
        :service="service"
        layout="card"
        @delete="onServiceDelete"
      />
    </div>
  </n-card>
</template>

<style scoped>
.pending-services-card {
  margin-bottom: 1rem;
  border: 1px solid var(--n-color-warning);
  background: var(--n-color-warning-light, rgba(255, 197, 61, 0.05));
}

.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
}

.header-title {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-weight: 600;
}

.header-icon {
  color: var(--n-color-warning);
}

.header-hint {
  font-size: 0.75rem;
  color: var(--n-text-color-3);
}

.services-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  gap: 1rem;
}
</style>
