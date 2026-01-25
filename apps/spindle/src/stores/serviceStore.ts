import { defineStore } from "pinia";
import { useTauriStore } from "./tauriStore";
import { ref } from "vue";

export const useServiceStore = defineStore("service", () => {
    const tauriStore = useTauriStore()

    const changeTrigger = ref(0);

    async function getGroupAlias(groupSha256Values: string): Promise<string | null> {
        const servicesStore = await tauriStore.getStore("services.json");
        const groupAlias = await servicesStore.get<string>(groupSha256Values);
        return groupAlias || null;
    }

    async function setGroupAlias(groupSha256Values: string, groupAlias: string): Promise<void> {
        const servicesStore = await tauriStore.getStore("services.json");
        await servicesStore.set(groupSha256Values, groupAlias);
        await servicesStore.save();
        ++changeTrigger.value;
    }

    async function removeGroupAlias(groupSha256Values: string): Promise<void> {
        const servicesStore = await tauriStore.getStore("services.json");
        if (await servicesStore.has(groupSha256Values)) {
            await servicesStore.delete(groupSha256Values);
            await servicesStore.save();
            ++changeTrigger.value;
        }
    }

    return { getGroupAlias, setGroupAlias, removeGroupAlias, changeTrigger }
})