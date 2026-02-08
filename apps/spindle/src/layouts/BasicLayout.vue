<!--
  BasicLayout: App shell with header, sidebar (service groups), and main content.
  Starts global service polling on mount; child views may also call start/stop polling.
-->
<script setup lang="ts">
import { onMounted } from "vue";
import { useRouter } from "vue-router";
import ThemeToggle from "@/components/ThemeToggle.vue";
import { NLayout, NLayoutHeader, NLayoutContent, NLayoutSider, NFlex, NCard } from "naive-ui";
import { useServiceStore } from "@/stores/serviceStore";

const router = useRouter();
const store = useServiceStore();

onMounted(() => {
  store.startPolling();
});

/** Navigate to the services overview page. */
function goServices() {
  router.push({ name: "Services" });
}

/**
 * Navigate to a specific service group detail page.
 * @param groupId - The group id (used as route param).
 */
function goGroup(groupId: number) {
  router.push({ name: "ServiceGroupDetail", params: { groupId: String(groupId) } });
}
</script>

<template>
  <n-layout position="absolute">
    <n-layout-header bordered class="header">
      <div class="header-flex-wrapper">
        <n-flex>
          <h1>SPINDLE</h1>
        </n-flex>
      </div>
      <div class="header-theme-toggle-wrapper">
        <ThemeToggle />
      </div>
    </n-layout-header>
    <n-layout has-sider sider-placement="left" class="body-layout" position="absolute">
      <n-layout-sider bordered class="sider" :native-scrollbar="false" show-trigger width="12rem" :collapsed-width="24">
        <n-flex vertical :size="8" class="sider-content">
          <n-card size="small" :bordered="false" class="sider-card" @click="goServices">
            <strong>Services</strong>
          </n-card>
          <n-card v-for="g in store.groups" :key="g.group_id" size="small" :bordered="false"
            class="sider-card sider-card-secondary" @click="goGroup(g.group_id)">
            {{ g.displayName }}
          </n-card>
        </n-flex>
      </n-layout-sider>
      <n-layout-content :native-scrollbar="false" class="body-content">
        <router-view v-slot="{ Component }">
          <transition name="fade" mode="out-in">
            <component :is="Component" />
          </transition>
        </router-view>
      </n-layout-content>
    </n-layout>
  </n-layout>
</template>

<style lang="css" scoped>
.header {
  display: flex;
  height: 3rem;
  align-items: center;
}

.body-layout {
  top: 3rem;
  bottom: 0;
}

.sider {
  height: 100%;
}

.sider-content {
  padding: 0.5rem;
}

.sider-content :deep(.sider-card) {
  cursor: pointer;
  border: none;
  border-radius: 6px;
}

.sider-content :deep(.sider-card-secondary) {
  margin-left: 0.5rem;
  padding-left: 0.5rem;
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.15s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.header-flex-wrapper {
  flex: 1;
  margin-left: 2rem;
}

.header-theme-toggle-wrapper {
  display: flex;
  height: 60%;
  max-height: 1.5rem;
  margin-right: 3vw;
  aspect-ratio: 1;
  align-items: center;
  justify-content: center;
}

.body-content {
  padding: 1rem;
}
</style>