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

    const DEFAULT_SERVICE_DIR = "./services";
    async function getServiceDirs() {
        const servicesStore = await tauriStore.getStore("services.json");
        const serviceDirs = await servicesStore.get("service_dirs");
        if (serviceDirs) {
            return serviceDirs;
        } else {
            await servicesStore.set("service_dirs", [DEFAULT_SERVICE_DIR]);
            await servicesStore.save();
            return [DEFAULT_SERVICE_DIR];
        }
    }

    async function addServiceDir(serviceDir: string) {
        const servicesStore = await tauriStore.getStore("services.json");
        let serviceDirs = await servicesStore.get<string[]>("service_dirs");
        if (serviceDirs) {
            serviceDirs.push(serviceDir);
        } else {
            serviceDirs = [serviceDir];
        }
        await servicesStore.set("service_dirs", serviceDirs);
        await servicesStore.save();
        ++changeTrigger.value;
    }

    async function removeServiceDir(serviceDir: string) {
        const servicesStore = await tauriStore.getStore("services.json");
        let serviceDirs = await servicesStore.get<string[]>("service_dirs");
        if (serviceDirs) {
            serviceDirs = serviceDirs.filter(dir => dir !== serviceDir);
        } else {
            serviceDirs = [DEFAULT_SERVICE_DIR];
        }
        await servicesStore.set("service_dirs", serviceDirs);
        await servicesStore.save();
        ++changeTrigger.value;
    }

    return { getGroupAlias, setGroupAlias, removeGroupAlias, changeTrigger, getServiceDirs, addServiceDir, removeServiceDir }
})