import { defineStore } from "pinia";
import { load, Store } from "@tauri-apps/plugin-store";

export const useTauriStore = defineStore("tauri", () => {
    async function getStore(fileName: string): Promise<Store> {
        const store = await load(fileName);
        return store;
    }
    return { getStore }
})