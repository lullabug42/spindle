<!--
  ServiceDetailModal: Card-style modal showing service name, version, status and dependencies.
  Requires group context to resolve dependency names/versions/status.
-->
<script setup lang="ts">
import { NModal, NDescriptions, NDescriptionsItem, NTag, NIcon } from "naive-ui";
import { CheckCircleFilled, ErrorFilled, StopFilled } from "@vicons/material";
import { computed } from "vue";
import type { ServiceItem as ServiceItemType, GroupWithStatus } from "@/types/service.types";

const props = defineProps<{
  /** Whether the modal is visible. */
  visible: boolean;
  /** The service to show; null when closed. */
  service: ServiceItemType | null;
  /** Group context to resolve dependency names, versions, and status. */
  group: GroupWithStatus | null;
}>();

const emit = defineEmits<{
  (e: "update:visible", v: boolean): void;
}>();

/** Naive UI tag type from service status. */
const statusType = computed(() => {
  if (!props.service) return "default";
  const s = props.service.status;
  if (s === "Running") return "success";
  if (s === "Error") return "error";
  return "default";
});

/** Icon component for current status. */
const StatusIcon = computed(() => {
  if (!props.service) return StopFilled;
  const s = props.service.status;
  if (s === "Running") return CheckCircleFilled;
  if (s === "Error") return ErrorFilled;
  return StopFilled;
});

/** Resolved dependency list (name, version, status) from group.services. */
const dependencyList = computed(() => {
  const svc = props.service;
  const grp = props.group;
  if (!svc || !grp || svc.dependency_ids.length === 0) return [];
  const byId = new Map(grp.services.map((s) => [s.service_id, s]));
  return svc.dependency_ids
    .map((id) => byId.get(id))
    .filter((dep): dep is ServiceItemType => dep != null)
    .map((dep) => ({
      name: dep.name,
      version: dep.version,
      status: dep.status,
    }));
});

function onClose() {
  emit("update:visible", false);
}
</script>

<template>
  <n-modal :show="visible" :on-update:show="(v: boolean) => emit('update:visible', v)" preset="card"
    title="Service Detail" style="width: 480px" @close="onClose">
    <template v-if="service">
      <n-descriptions label-placement="left" :column="1">
        <n-descriptions-item label="Name">
          {{ service.name }}
        </n-descriptions-item>
        <n-descriptions-item label="Version">
          {{ service.version }}
        </n-descriptions-item>
        <n-descriptions-item label="Status">
          <n-tag :type="statusType" size="small" class="status-tag">
            <span class="status-tag-inner">
              <n-icon :component="StatusIcon" size="14" />
              <span>{{ service.status }}</span>
            </span>
          </n-tag>
        </n-descriptions-item>
      </n-descriptions>
      <div v-if="dependencyList.length" class="deps-section">
        <div class="deps-title">Dependencies</div>
        <ul class="deps-list">
          <li v-for="d in dependencyList" :key="`${d.name}-${d.version}`" class="dep-item">
            <span class="dep-name">{{ d.name }}</span>
            <span class="dep-version">v{{ d.version }}</span>
            <n-tag size="tiny" :type="d.status === 'Running' ? 'success' : d.status === 'Error' ? 'error' : 'default'">
              {{ d.status }}
            </n-tag>
          </li>
        </ul>
      </div>
    </template>
  </n-modal>
</template>

<style scoped>
.status-tag-inner {
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
}

.deps-section {
  margin-top: 1rem;
  padding-top: 1rem;
  border-top: 1px solid var(--n-border-color);
}

.deps-title {
  font-weight: 600;
  margin-bottom: 0.5rem;
}

.deps-list {
  list-style: none;
  padding: 0;
  margin: 0;
}

.dep-item {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.35rem 0;
}

.dep-name {
  font-weight: 500;
  min-width: 8rem;
}

.dep-version {
  color: var(--n-text-color-3);
  font-size: 0.9rem;
}
</style>
