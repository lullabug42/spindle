import { defineStore } from "pinia";
import { ref } from "vue";
import { load } from "@tauri-apps/plugin-store";

export const useThemeStore = defineStore("theme", () => {
    const systemTheme = window.matchMedia('(prefers-color-scheme: dark)').matches ? "dark" : "light"
    const theme = ref(systemTheme)

    async function initTheme() {
        const preferences = await load("preferences.json");
        const lastTheme = await preferences.get<string>("theme");
        if (lastTheme) {
            theme.value = lastTheme;
        }
    }
    async function toggleTheme() {
        theme.value = theme.value === "dark" ? "light" : "dark"
        const preferences = await load("preferences.json");
        await preferences.set("theme", theme.value)
        await preferences.save()
    }
    return { theme, initTheme, toggleTheme }
})