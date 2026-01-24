import { createApp } from "vue";
import App from "./App.vue";
import { createPinia } from "pinia";
import { useTauriStore } from "./stores/tauriStore";
import { useThemeStore } from "./stores/themeStore";
import router from "@/router";

const app = createApp(App)
const pinia = createPinia()

app.use(pinia)
await useTauriStore().initTauriStore()
await useThemeStore().initTheme()

app.use(router)

app.mount("#app")