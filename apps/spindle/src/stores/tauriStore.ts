import { defineStore } from "pinia";
import { load, Store } from "@tauri-apps/plugin-store";
import { shallowRef } from "vue";

export const useTauriStore = defineStore("tauri", () => {
    const preferences = shallowRef<Store | null>(null)
    async function initTauriStore() {
        if (!preferences.value) {
            preferences.value = await load("preferences.json")
        }
    }
    return { preferences, initTauriStore }
})