import { defineStore } from "pinia";
import { ref } from "vue";
import { useTauriStore } from "./tauriStore";

export const useThemeStore = defineStore("theme", () => {
    const systemTheme = window.matchMedia('(prefers-color-scheme: dark)').matches ? "dark" : "light"
    const theme = ref(systemTheme)
    const tauriStore = useTauriStore()

    async function initTheme() {
        const preferences = tauriStore.preferences
        if (preferences) {
            const lastTheme = await preferences.get<string>("theme")
            if (lastTheme) {
                theme.value = lastTheme
            }
        }
    }
    async function toggleTheme() {
        const preferences = tauriStore.preferences
        theme.value = theme.value === "dark" ? "light" : "dark"
        if (preferences) {
            console.log("preferences", preferences)
            await preferences.set("theme", theme.value)
            await preferences.save()
            console.log("preferences", preferences)
        }
    }
    return { theme, initTheme, toggleTheme }
})