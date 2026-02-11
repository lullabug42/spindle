<!--
  ServiceItem: Single service display in card or list layout.
  Shows name, version, status (tag + icon); emits click for detail.
  Provides delete functionality with modal confirmation.
-->
<script setup lang="ts">
import { CheckCircleFilled, ErrorFilled, StopFilled, DeleteOutlineOutlined } from "@vicons/material";
import { NCard, NIcon, NTag, NSpace, NButton, NModal, NTooltip } from "naive-ui";
import { computed, ref } from "vue";
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
  (e: "delete", service: ServiceItemType): void;
}>();

/** Controls visibility of the delete confirmation modal. */
const showDeleteModal = ref(false);

/** Status string -> tag type and icon component (single source of truth). */
const STATUS_CONFIG: Record<string, { type: "success" | "error" | "default"; icon: Component }> = {
  Running: { type: "success", icon: CheckCircleFilled },
  Error: { type: "error", icon: ErrorFilled },
};

const statusConfig = computed(() => STATUS_CONFIG[props.service.status] ?? { type: "default" as const, icon: StopFilled });

function onClick() {
  emit("click", props.service);
}

function openDeleteModal() {
  showDeleteModal.value = true;
}

function closeDeleteModal() {
  showDeleteModal.value = false;
}

function confirmDelete() {
  showDeleteModal.value = false;
  emit("delete", props.service);
}
</script>

<template>
  <div>
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
          <div class="info-row">
            <span class="version-label">Version</span>
            <n-tooltip trigger="hover">
              <template #trigger>
                <span class="version">v{{ service.version }}</span>
              </template>
              {{ service.version }}
            </n-tooltip>
          </div>
          <div class="info-row">
            <span class="id-label">ID</span>
            <span class="service-id">{{ service.service_id }}</span>
          </div>
        </div>
        <n-space class="actions" @click.stop>
          <n-button quaternary circle size="small" class="delete-btn" @click="openDeleteModal">
            <template #icon>
              <n-icon :component="DeleteOutlineOutlined" />
            </template>
          </n-button>
        </n-space>
      </div>
    </n-card>

    <!-- List mode -->
    <div v-else class="service-item-list" @click="onClick">
      <div class="list-left">
        <div class="list-name-row">
          <span class="service-name">{{ service.name }}</span>
          <span class="list-service-id">#{{ service.service_id }}</span>
        </div>
        <n-tooltip trigger="hover">
        <template #trigger>
          <span class="version list-version">v{{ service.version }}</span>
        </template>
        {{ service.version }}
      </n-tooltip>
      </div>
      <div class="list-center">
        <n-tag :type="statusConfig.type" size="small">
          <n-icon :component="statusConfig.icon" size="14" />
          {{ service.status }}
        </n-tag>
      </div>
      <div class="list-right" @click.stop>
        <n-space>
          <n-button quaternary circle size="small" class="delete-btn" @click="openDeleteModal">
            <template #icon>
              <n-icon :component="DeleteOutlineOutlined" />
            </template>
          </n-button>
        </n-space>
      </div>
    </div>

    <!-- Delete Confirmation Modal -->
    <n-modal
      v-model:show="showDeleteModal"
      preset="card"
      title="Confirm Deletion"
      style="width: 24rem"
      :mask-closable="false"
    >
      <p class="delete-message">
        Are you sure you want to delete
        <strong>{{ service.name }}:{{ service.version }}</strong>?
      </p>
      <p class="delete-hint">
        This service will be permanently removed. If other services depend on it, the deletion will be blocked.
      </p>
      <template #footer>
        <n-space justify="end">
          <n-button @click="closeDeleteModal">Cancel</n-button>
          <n-button type="error" @click="confirmDelete">Delete</n-button>
        </n-space>
      </template>
    </n-modal>
  </div>
</template>

<style scoped>
.service-item-card {
  cursor: pointer;
  min-width: 14rem;
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
  flex-direction: column;
  gap: 0.25rem;
  min-width: 0;
  flex: 1;
}

.info-row {
  display: flex;
  align-items: baseline;
  gap: 0.5rem;
  min-width: 0;
}

.version-label,
.id-label {
  color: var(--n-text-color-3);
  font-size: 0.75rem;
  min-width: 3rem;
}

.version {
  color: var(--n-text-color-2);
  font-size: 0.875rem;
  font-family: ui-monospace, monospace;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 8rem;
  display: block;
}

.service-id {
  color: var(--n-text-color-3);
  font-size: 0.875rem;
  font-family: ui-monospace, monospace;
}

.actions {
  flex-shrink: 0;
}

.delete-btn {
  opacity: 0.5;
  transition: opacity 0.2s;
}

.delete-btn:hover {
  opacity: 1;
}

.service-item-list {
  display: flex;
  align-items: center;
  height: 3.5rem;
  min-width: 14rem;
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

.list-name-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  min-width: 0;
}

.list-service-id {
  color: var(--n-text-color-3);
  font-size: 0.75rem;
  font-family: ui-monospace, monospace;
  flex-shrink: 0;
}

.list-center {
  flex-shrink: 0;
  margin: 0 1rem;
}

.list-right {
  flex-shrink: 0;
}

.delete-message {
  margin: 0 0 0.5rem 0;
  font-size: 0.875rem;
  line-height: 1.5;
}

.delete-hint {
  margin: 0;
  font-size: 0.75rem;
  color: var(--n-text-color-3);
  line-height: 1.5;
}
</style>
