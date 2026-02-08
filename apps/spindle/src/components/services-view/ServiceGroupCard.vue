<!--
  ServiceGroupCard: Card for one service group with header (name, status tag, launch/stop),
  and a grid or list of ServiceItems. Clicking the card navigates to group detail.
-->
<script setup lang="ts">
import { NCard, NButton, NTag, NSpace } from "naive-ui";
import { computed } from "vue";
import { useRouter } from "vue-router";
import ServiceItem from "./ServiceItem.vue";
import type { GroupWithStatus, ServiceItem as ServiceItemType } from "@/types/service.types";
import { useServiceStore } from "@/stores/serviceStore";

const props = defineProps<{
  /** The group to display. */
  group: GroupWithStatus;
  /** Layout for services: grid of cards or list. */
  viewMode: "grid" | "list";
}>();

const router = useRouter();
const store = useServiceStore();

/** Number of services with status "Running"; computed once and reused for summary and status. */
const runningCount = computed(() =>
  props.group.services.filter((s) => s.status === "Running").length
);

const serviceSummaryText = computed(() => {
  const total = props.group.services.length;
  const running = runningCount.value;
  if (total === 0) return "No services";
  if (running === total) return `${total} services, all running`;
  if (running === 0) return `${total} services, none running`;
  return `${total} services, ${running} running`;
});

type GroupStatusKind = "success" | "warning" | "error" | "default";
/** Group status derived from running count and error presence (reuses runningCount). */
const groupStatus = computed((): GroupStatusKind => {
  const total = props.group.services.length;
  if (total === 0) return "default";
  const running = runningCount.value;
  const hasError = props.group.services.some((s) => s.status === "Error");
  if (hasError) return "error";
  if (running === total) return "success";
  if (running > 0) return "warning";
  return "default";
});

const headerBorderClass = computed(() => `header-border-${groupStatus.value}`);

/** Placeholder: set to true while launch request is in flight to show loading on button. */
const launching = computed(() => false);

async function launch() {
  await store.launchGroup(props.group.group_id);
}

async function stop() {
  await store.stopGroup(props.group.group_id);
}

const emit = defineEmits<{
  (e: "service-click", service: ServiceItemType): void;
}>();

function goDetail() {
  router.push({ name: "ServiceGroupDetail", params: { groupId: String(props.group.group_id) } });
}

function onServiceClick(service: ServiceItemType) {
  emit("service-click", service);
}
</script>

<template>
  <n-card class="group-card" @click="goDetail">
    <template #header>
      <div class="group-header" :class="headerBorderClass">
        <div class="header-left">
          <span class="group-name">{{ group.displayName }}</span>
          <n-tag size="small" :type="groupStatus" round>
            {{ serviceSummaryText }}
          </n-tag>
        </div>
        <n-space class="header-right" @click.stop>
          <n-button size="small" type="primary" :loading="launching" @click="launch">
            Launch
          </n-button>
          <n-button size="small" @click="stop">Stop</n-button>
        </n-space>
      </div>
    </template>
    <div class="group-body" :class="viewMode === 'grid' ? 'body-grid' : 'body-list'" @click.stop>
      <ServiceItem v-for="svc in group.services" :key="`${svc.name}-${svc.version}`" :service="svc"
        :layout="viewMode === 'grid' ? 'card' : 'list'" @click="onServiceClick" />
    </div>
  </n-card>
</template>

<style scoped>
.group-card {
  min-width: 0;
}

.group-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding-left: 0.5rem;
  border-left-width: 4px;
  border-left-style: solid;
}

.header-border-success {
  border-left-color: var(--n-color-success);
  background: color-mix(in srgb, var(--n-color-success) 15%, transparent);
}

.header-border-warning {
  border-left-color: var(--n-color-warning);
  background: color-mix(in srgb, var(--n-color-warning) 15%, transparent);
}

.header-border-error {
  border-left-color: var(--n-color-error);
  background: color-mix(in srgb, var(--n-color-error) 15%, transparent);
}

.header-border-default {
  border-left-color: var(--n-border-color);
  background: var(--n-color-modal);
}

.header-left {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  min-width: 0;
}

.group-name {
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.header-right {
  flex-shrink: 0;
}

.body-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  gap: 1rem;
}

.body-list {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}
</style>
