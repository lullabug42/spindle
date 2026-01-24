<script setup lang="ts">
import ThemeToggle from "@/components/ThemeToggle.vue";
import { NLayout, NLayoutHeader, NLayoutContent, NLayoutSider, NFlex } from "naive-ui";
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
            <n-layout-sider bordered class="sider" :native-scrollbar="false" show-trigger width="12rem"
                :collapsed-width=24>
                <n-flex vertical>
                </n-flex>
            </n-layout-sider>
            <n-layout-content :native-scrollbar="false" class="body-content">
                <router-view />
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