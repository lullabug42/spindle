import { useServiceStore } from "@/stores/serviceStore";
import { GroupServiceInfo } from "@/types/service.types";
import { invoke } from "@tauri-apps/api/core";

export async function getServiceGroupInfos(): Promise<GroupServiceInfo[]> {
    return await invoke<GroupServiceInfo[]>("service_group_infos");
}

export async function getServiceGroupNum(): Promise<number> {
    return await invoke<number>("service_group_num");
}

export async function reloadServiceManager() {
    const serviceStore = useServiceStore();
    await invoke("reload_service_manager", { service_dirs: await serviceStore.getServiceDirs() });
}