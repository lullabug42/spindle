<script setup lang="ts">
import { darkTheme, lightTheme, NConfigProvider, NGlobalStyle, NMessageProvider } from "naive-ui";
import { computed } from "vue";
import BasicLayout from "@/layouts/BasicLayout.vue";
import { useThemeStore } from "@/stores/themeStore";

const themeStore = useThemeStore()
const theme = computed(() => themeStore.theme === "dark" ? darkTheme : lightTheme)
</script>

<template>
  <n-config-provider :theme="theme">
    <n-global-style />
    <n-message-provider>
      <BasicLayout />
    </n-message-provider>
  </n-config-provider>
</template>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html,
body {
  width: 100%;
  height: 100%;
}

#app {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
}
</style>