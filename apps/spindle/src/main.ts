import { createApp } from "vue";
import App from "./App.vue";
import { createPinia } from "pinia";
import { useThemeStore } from "./stores/themeStore";
import router from "@/router";

const app = createApp(App);
const pinia = createPinia();

app.use(pinia);

(async () => {
  await useThemeStore().initTheme();
  app.use(router);
  app.mount("#app");
})();