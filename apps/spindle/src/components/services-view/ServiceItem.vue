<!--
  ServiceItem: Single service display in card or list layout.
  Shows name, version, status (tag + icon); emits click for detail.
-->
<script setup lang="ts">
import { CheckCircleFilled, ErrorFilled, StopFilled } from "@vicons/material";
import { NCard, NIcon, NTag, NSpace } from "naive-ui";
import { computed } from "vue";
import type { Component } from "vue";
import type { ServiceItem as ServiceItemType } from "@/types/service.types";

const props = withDefaults(
  defineProps<{
    /** The service to display. */
    service: ServiceItemType;
    /** Card (default) or list layout. */
    layout?: "card" | "list";
  }>(),
  { layout: "card" }
);

const emit = defineEmits<{
  (e: "click", service: ServiceItemType): void;
}>();

/** Status string -> tag type and icon component (single source of truth). */
const STATUS_CONFIG: Record<string, { type: "success" | "error" | "default"; icon: Component }> = {
  Running: { type: "success", icon: CheckCircleFilled },
  Error: { type: "error", icon: ErrorFilled },
};

const statusConfig = computed(() => STATUS_CONFIG[props.service.status] ?? { type: "default" as const, icon: StopFilled });

function onClick() {
  emit("click", props.service);
}
</script>

<template>
  <!-- Card mode -->
  <n-card v-if="layout === 'card'" class="service-item-card" hoverable @click="onClick">
    <template #header>
      <div class="card-header-inner">
        <span class="service-name">{{ service.name }}</span>
        <n-tag :type="statusConfig.type" size="small" class="status-tag">
          <span class="status-tag-inner">
            <n-icon :component="statusConfig.icon" size="14" />
            <span>{{ service.status }}</span>
          </span>
        </n-tag>
      </div>
    </template>
    <div class="card-body">
      <div class="card-body-main">
        <span class="version-label">Version</span>
        <span class="version">v{{ service.version }}</span>
      </div>
      <n-space class="actions" @click.stop>
        <!-- Actions placeholder for future: start/stop -->
      </n-space>
    </div>
  </n-card>

  <!-- List mode -->
  <div v-else class="service-item-list" @click="onClick">
    <div class="list-left">
      <span class="service-name">{{ service.name }}</span>
      <span class="version">v{{ service.version }}</span>
    </div>
    <div class="list-center">
      <n-tag :type="statusConfig.type" size="small">
        <n-icon :component="statusConfig.icon" size="14" />
        {{ service.status }}
      </n-tag>
    </div>
    <div class="list-right" @click.stop>
      <n-space>
        <!-- Actions placeholder -->
      </n-space>
    </div>
  </div>
</template>

<style scoped>
.service-item-card {
  cursor: pointer;
  min-width: 12rem;
  min-height: 4.5rem;
}

.service-item-card :deep(.n-card-header) {
  padding-right: 0.75rem;
}

.card-header-inner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  width: 100%;
  min-width: 0;
}

.service-name {
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
  min-width: 0;
}

.status-tag {
  flex-shrink: 0;
}

.status-tag-inner {
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
}

.card-body {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  padding: 0.25rem 0;
  min-height: 2rem;
}

.card-body-main {
  display: flex;
  align-items: baseline;
  gap: 0.5rem;
}

.version-label {
  color: var(--n-text-color-3);
  font-size: 0.8rem;
}

.version {
  color: var(--n-text-color-2);
  font-size: 0.9rem;
  font-family: ui-monospace, monospace;
}

.service-item-list {
  display: flex;
  align-items: center;
  height: 3.5rem;
  min-width: 10rem;
  padding: 0 1rem;
  cursor: pointer;
  border-radius: 6px;
  transition: background-color 0.2s;
}

.service-item-list:hover {
  background-color: var(--n-color-hover);
}

.list-left {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.list-center {
  flex-shrink: 0;
  margin: 0 1rem;
}

.list-right {
  flex-shrink: 0;
}
</style>
