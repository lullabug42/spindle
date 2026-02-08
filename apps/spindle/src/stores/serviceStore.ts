/**
 * Pinia store for service groups and polling.
 *
 * @remarks
 * When useMock is true, uses {@link mockGroupsForDev} (all topology scenarios).
 * To use only the four showcase groups, import `mockGroups` instead and set `groups.value = mockGroups`.
 */
import { defineStore } from "pinia";
import { ref, computed } from "vue";
import * as serviceApi from "@/services/service";
import type { GroupInfo, GroupWithStatus, ServiceItem } from "@/types/service.types";
import type { StoredServiceConfig } from "@/types/service.types";
import { mockGroupsForDev } from "@/mocks/serviceMock";

/** Polling interval in milliseconds. */
const POLL_INTERVAL_MS = 3000;

/** Adds status to a stored config to produce a {@link ServiceItem}. */
function withStatus(config: StoredServiceConfig, status: string): ServiceItem {
  return { ...config, status };
}

/**
 * Fetches runtime state for one service.
 * @returns Status string, or "Stopped" on error.
 */
async function fetchServiceState(name: string, version: string): Promise<string> {
  try {
    return await serviceApi.serviceState({ name, version });
  } catch {
    return "Stopped";
  }
}

/**
 * Enriches {@link GroupInfo} with per-service status and display name.
 * @param info - Raw group info from API.
 * @param displayName - Display name (alias or "Unnamed Group N").
 */
async function enrichGroup(
  info: GroupInfo,
  displayName: string
): Promise<GroupWithStatus> {
  const statuses = await Promise.all(
    info.services.map((s) => fetchServiceState(s.name, s.version))
  );
  const services: ServiceItem[] = info.services.map((s, i) =>
    withStatus(s, statuses[i] ?? "Stopped")
  );
  return {
    group_id: info.group_id,
    alias: info.alias,
    displayName,
    services: services.sort((a, b) => a.name.localeCompare(b.name)),
  };
}

/**
 * Merges aliased and unaliased groups: aliased sorted by alias, unaliased by group_id.
 * Assigns displayName (alias for aliased, "Unnamed Group N" for unaliased).
 */
function mergeAndSortGroups(aliased: GroupInfo[], unaliased: GroupInfo[]): { info: GroupInfo; displayName: string }[] {
  const aliasedSorted = [...aliased].sort((a, b) =>
    (a.alias ?? "").localeCompare(b.alias ?? "")
  );
  const unaliasedSorted = [...unaliased].sort((a, b) => a.group_id - b.group_id);
  return [
    ...aliasedSorted.map((info) => ({
      info,
      displayName: info.alias ?? "",
    })),
    ...unaliasedSorted.map((info, index) => ({
      info,
      displayName: `Unnamed Group ${index}`,
    })),
  ];
}

export const useServiceStore = defineStore("service", () => {
  /** All groups with status (mock or from API). */
  const groups = ref<GroupWithStatus[]>([]);
  /** True while a fetch is in progress. */
  const loading = ref(false);
  /** True when current data is from mock. */
  const useMock = ref(false);
  /** Overview page view: grid or list. */
  const overviewViewMode = ref<"grid" | "list">("grid");
  /** Group detail page view: card or graph. */
  const detailViewMode = ref<"card" | "graph">("card");

  let pollTimer: ReturnType<typeof setInterval> | null = null;

  /** Fetches groups (mock or API) and updates {@link groups}. */
  async function fetchGroups() {
    loading.value = true;
    if (useMock.value) {
      groups.value = mockGroupsForDev;
      loading.value = false;
      return;
    }
    try {
      const [aliased, unaliased] = await Promise.all([
        serviceApi.aliasedGroupService(),
        serviceApi.unaliasedGroupService(),
      ]);
      if (aliased.length > 0 || unaliased.length > 0) {
        const ordered = mergeAndSortGroups(aliased, unaliased);
        const enriched = await Promise.all(
          ordered.map(({ info, displayName }) => enrichGroup(info, displayName))
        );
        groups.value = enriched;
        useMock.value = false;
      } else {
        groups.value = [];
        useMock.value = false;
      }
    } catch {
      groups.value = [];
      useMock.value = false;
    } finally {
      loading.value = false;
    }
  }

  /** Starts periodic fetch; no-op if already running. */
  function startPolling() {
    if (pollTimer) return;
    void fetchGroups();
    pollTimer = setInterval(fetchGroups, POLL_INTERVAL_MS);
  }

  /** Stops periodic fetch. */
  function stopPolling() {
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  }

  /**
   * Sets whether to use mock data.
   * When switching to mock: stops polling (mock data is static).
   * When switching to real data: calls {@link reloadServiceManager}, fetches once, then starts polling.
   */
  async function setUseMock(value: boolean) {
    useMock.value = value;
    if (value) {
      stopPolling();
    } else {
      await serviceApi.reloadServiceManager();
      startPolling();
      return;
    }
    await fetchGroups();
  }

  /** Launches a group by id and then refetches groups. */
  async function launchGroup(groupId: number) {
    await serviceApi.launchGroup({ group_id: groupId, timeout_ms: 30_000 });
    await fetchGroups();
  }

  /** Stops a group by id and then refetches groups. */
  async function stopGroup(groupId: number) {
    await serviceApi.stopGroup({ group_id: groupId });
    await fetchGroups();
  }

  /** Map of group_id to group for O(1) lookup. */
  const groupById = computed(() => {
    const map = new Map<number, GroupWithStatus>();
    for (const g of groups.value) map.set(g.group_id, g);
    return map;
  });

  return {
    groups,
    loading,
    useMock,
    overviewViewMode,
    detailViewMode,
    fetchGroups,
    startPolling,
    stopPolling,
    setUseMock,
    launchGroup,
    stopGroup,
    groupById,
  };
});
