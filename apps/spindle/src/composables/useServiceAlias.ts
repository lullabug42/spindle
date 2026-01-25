import { useServiceStore } from "@/stores/serviceStore";
import { GroupServiceInfo } from "@/types/service.types";
import { toHash } from "@/utils/serviceHash";
import { computed, MaybeRefOrGetter, ref, toValue, watch } from "vue";


export function useServiceAlias(groupServiceInfos: MaybeRefOrGetter<GroupServiceInfo[]>) {
    const groupSha256Values = computed(() => toValue(groupServiceInfos).map(toHash));

    const groupAliasMap = ref(new Map<string, string>());
    const serviceStore = useServiceStore();
    watch([groupSha256Values, () => serviceStore.changeTrigger], async ([newGroupSha256Values, _trigger], _old, onCleanUp) => {
        let isCancelled = false;
        onCleanUp(() => isCancelled = true);
        const newAliasMap = new Map<string, string>();
        const promises = newGroupSha256Values.map(async (hash) => {
            const alias = await serviceStore.getGroupAlias(hash);
            if (alias) {
                newAliasMap.set(hash, alias);
            }
        });
        await Promise.all(promises);
        if (!isCancelled) {
            groupAliasMap.value = newAliasMap;
        }
    }, { immediate: true });

    const aliasedGroupIdxs = computed(() => {
        const idxs = new Array<number>();
        groupSha256Values.value.forEach((val, idx) => {
            if (groupAliasMap.value.has(val)) {
                idxs.push(idx);
            }
        });
        return idxs;
    })

    const unaliasedGroupIdxs = computed(() => {
        const idxs = new Array<number>();
        groupSha256Values.value.forEach((val, idx) => {
            if (!groupAliasMap.value.has(val)) {
                idxs.push(idx);
            }
        });
        return idxs;
    })


    return { groupAliasMap, groupSha256Values, aliasedGroupIdxs, unaliasedGroupIdxs };
}
