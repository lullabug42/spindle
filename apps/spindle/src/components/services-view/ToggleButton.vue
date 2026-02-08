<!--
  ToggleButton: Toggles between two view modes (e.g. grid and graph).
  Expects v-model:view-mode to be "grid" or "graph"; toggles on click.
-->
<script setup lang="ts">
import { GridViewOutlined, HubOutlined } from "@vicons/material";
import { NButton, NIcon, NText } from "naive-ui";
import { computed } from "vue";

const model = defineModel("view-mode", { type: String, required: true });
const props = defineProps({
  /** Size of the icon (CSS length or number). */
  iconSize: { type: [Number, String], default: "1.5rem" },
});

const currentIcon = computed(() =>
  model.value === "grid" ? GridViewOutlined : HubOutlined
);

const toggle = () => {
  model.value = model.value === "grid" ? "graph" : "grid";
};
</script>

<template>
  <n-button @click="toggle" text :focusable="false" class="smooth-hover-btn">
    <n-icon :size="props.iconSize">
      <Transition name="icon-fade" mode="out-in">
        <component :is="currentIcon" :key="model" />
      </Transition>
    </n-icon>
    <Transition name="text-fade" mode="out-in">
      <n-text class="toggle-button-text" :key="model">{{ model === 'grid' ? 'Grid' : 'Graph' }}</n-text>
    </Transition>
  </n-button>
</template>

<style lang="css" scoped>
.smooth-hover-btn {
  transition: color 0.5s cubic-bezier(0.4, 0, 0.2, 1);
}

.icon-fade-enter-active,
.icon-fade-leave-active {
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}

.icon-fade-enter-from,
.icon-fade-leave-to {
  opacity: 0;
  transform: scale(0.5) rotate(90deg);
}

.toggle-button-text {
  font-size: 1rem;
  font-weight: 600;
  margin-left: 0.5rem;
  text-align: left;
  width: 6rem;
}

.text-fade-enter-active,
.text-fade-leave-active {
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}

.text-fade-enter-from,
.text-fade-leave-to {
  opacity: 0;
  transform: scale(0.5) rotate(90deg);
}

.text-fade-enter-from {
  opacity: 0;
  transform: translateY(10px);
}

.text-fade-leave-to {
  opacity: 0;
  transform: translateY(-10px);
}
</style>