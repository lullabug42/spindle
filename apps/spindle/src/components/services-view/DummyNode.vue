<!--
  DummyNode: Virtual DAG start/end node; card-style, display only (no interaction).
-->
<script setup lang="ts">
import { NCard, NTag } from "naive-ui";

defineProps<{
  /** Dummy node kind: start (root) or end (sink). */
  kind: "start" | "end";
}>();

const label = { start: "Start", end: "End" };
const tagType = { start: "success" as const, end: "info" as const };
</script>

<template>
  <n-card class="dummy-node" :class="`dummy-node--${kind}`" bordered>
    <template #header>
      <div class="dummy-node__header">
        <span class="dummy-node__label">{{ label[kind] }}</span>
        <n-tag :type="tagType[kind]" size="small" round>{{ label[kind] }}</n-tag>
      </div>
    </template>
    <div class="dummy-node__body">Virtual node</div>
  </n-card>
</template>

<style scoped>
.dummy-node {
  width: 100%;
  height: 100%;
  min-height: 96px;
  pointer-events: none;
}

.dummy-node :deep(.n-card-header) {
  padding-right: 0.75rem;
}

.dummy-node__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
}

.dummy-node__label {
  font-size: 14px;
  font-weight: 600;
  color: var(--n-text-color-1);
}

.dummy-node__body {
  font-size: 12px;
  color: var(--n-text-color-3);
}

.dummy-node--start .dummy-node__label {
  color: var(--n-color-success);
}

.dummy-node--end .dummy-node__label {
  color: var(--n-color-info);
}
</style>
