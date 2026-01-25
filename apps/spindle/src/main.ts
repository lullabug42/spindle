import { createApp } from "vue";
import App from "./App.vue";
import { createPinia } from "pinia";
import { useThemeStore } from "./stores/themeStore";
import router from "@/router";
import { reloadServiceManager } from "./services/service";

const app = createApp(App)
const pinia = createPinia()

app.use(pinia)
await useThemeStore().initTheme()

app.use(router)

await reloadServiceManager();

app.mount("#app")