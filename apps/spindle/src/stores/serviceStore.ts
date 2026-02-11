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
import type { GroupInfo, GroupWithStatus, ServiceItem, AddServiceParams, ServiceNameVersionParams } from "@/types/service.types";
import type { StoredServiceConfig } from "@/types/service.types";
import { mockGroupsForDev } from "@/mocks/serviceMock";

// ============================================================================
// Constants
// ============================================================================

/** Polling interval in milliseconds. */
const POLL_INTERVAL_MS = 3000;

// ============================================================================
// Helper Functions
// ============================================================================

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
  /** Services that have been added but not yet assigned to a group or reloaded. */
  const pendingServices = ref<ServiceItem[]>([]);
  /** True while a fetch is in progress. */
  const loading = ref(false);
  /** True when current data is from mock. */
  const useMock = ref(false);
  /** Overview page view: grid or list. */
  const overviewViewMode = ref<"grid" | "list">("grid");
  /** Group detail page view: card or graph. */
  const detailViewMode = ref<"card" | "graph">("card");

  let pollTimer: ReturnType<typeof setInterval> | null = null;

  /**
   * Fetches groups (mock or API) and updates {@link groups}.
   * In mock mode: only initializes data if groups.value is empty, otherwise preserves existing data.
   * This allows mock mode to support user modifications (add/remove services, launch/stop groups).
   */
  async function fetchGroups() {
    loading.value = true;
    if (useMock.value) {
      // In mock mode, only initialize if groups are empty
      // This preserves user modifications (added services, status changes from launch/stop)
      if (groups.value.length === 0) {
        // Deep clone to allow modifications without affecting the original mock data
        groups.value = JSON.parse(JSON.stringify(mockGroupsForDev));
      }
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

  /**
   * Launches services in a group in topological order (dependencies first).
   * Uses iterative algorithm: repeatedly start services whose dependencies are all started.
   * @param services - Array of services to launch.
   * @param serviceMap - Map of service_id -> ServiceItem for quick lookup.
   */
  function launchServicesInOrder(
    services: ServiceItem[],
    serviceMap: Map<number, ServiceItem>
  ): void {
    const started = new Set<number>();
    let changed = true;

    while (changed) {
      changed = false;
      for (const service of services) {
        if (started.has(service.service_id)) continue;

        // Check if all dependencies are started (or service has no dependencies)
        const allDepsStarted = service.dependency_ids.every((depId) => {
          const dep = serviceMap.get(depId);
          return dep && (started.has(depId) || dep.status === "Running");
        });

        if (allDepsStarted) {
          service.status = "Running";
          started.add(service.service_id);
          changed = true;
        }
      }
    }
  }

  /**
   * Launches a group by id and then refetches groups.
   * In mock mode: simulates launching by setting services to "Running" status in dependency order.
   *   Services with no dependencies are started first, then services whose dependencies are Running.
   * In real mode: calls the backend API to launch the group.
   */
  async function launchGroup(groupId: number) {
    if (useMock.value) {
      // Mock mode: simulate launching by updating service statuses in dependency order
      const group = groups.value.find((g) => g.group_id === groupId);
      if (!group) return;

      const serviceMap = new Map(
        group.services.map((s) => [s.service_id, s])
      );
      launchServicesInOrder(group.services, serviceMap);
      // No need to fetchGroups in mock mode since we updated state directly
      return;
    }
    // Real mode: call API and refresh
    await serviceApi.launchGroup({ group_id: groupId, timeout_ms: 30_000 });
    await fetchGroups();
  }

  /**
   * Stops a group by id and then refetches groups.
   * In mock mode: simulates stopping by setting all services in the group to "Stopped" status.
   *   This matches real behavior where stopping a group stops all services in that group.
   * In real mode: calls the backend API to stop the group (backend handles cascading).
   */
  async function stopGroup(groupId: number) {
    if (useMock.value) {
      // Mock mode: simulate stopping by setting all services in the group to Stopped
      const group = groups.value.find((g) => g.group_id === groupId);
      if (group) {
        // Stop all services in the group (simulating group-level stop)
        for (const service of group.services) {
          service.status = "Stopped";
        }
      }
      // No need to fetchGroups in mock mode since we updated state directly
      return;
    }
    // Real mode: call API and refresh
    await serviceApi.stopGroup({ group_id: groupId });
    await fetchGroups();
  }

  /** Map of group_id to group for O(1) lookup. */
  const groupById = computed(() => {
    const map = new Map<number, GroupWithStatus>();
    for (const g of groups.value) map.set(g.group_id, g);
    return map;
  });

  /** Counter for generating unique service IDs in mock mode. */
  let mockServiceIdCounter = 1000;

  /** True when service configuration has changed and needs to be rebuilt. */
  const hasConfigChanges = ref(false);

  /**
   * Checks if a service with the given name and version already exists.
   * @param name - Service name.
   * @param version - Service version.
   * @returns True if the service already exists.
   */
  function serviceExists(name: string, version: string): boolean {
    for (const group of groups.value) {
      if (group.services.some((s) => s.name === name && s.version === version)) {
        return true;
      }
    }
    return false;
  }

  /**
   * Finds a service ID by name and version across groups and pending services.
   * @param name - Service name.
   * @param version - Service version.
   * @returns Service ID if found, null otherwise.
   */
  function findServiceId(name: string, version: string): number | null {
    // Check groups first
    for (const group of groups.value) {
      const service = group.services.find(
        (s) => s.name === name && s.version === version
      );
      if (service) {
        return service.service_id;
      }
    }
    // Check pending services
    const pendingService = pendingServices.value.find(
      (s) => s.name === name && s.version === version
    );
    if (pendingService) {
      return pendingService.service_id;
    }
    return null;
  }

  /**
   * Adds a new service to the pending list.
   * In mock mode: generates a temporary ID and adds to pending.
   * In real mode: calls backend API to get the real service ID, then adds to pending.
   * Dependencies are resolved by converting (name, version) pairs to service_ids.
   * @param params - Service parameters for creation.
   * @returns Promise resolving to the created service ID.
   * @throws Error if a service with the same name and version already exists, or if a dependency is not found.
   */
  async function addServiceToStore(params: AddServiceParams): Promise<number> {
    // Check for duplicate name + version in both groups and pending services
    if (serviceExists(params.name, params.version)) {
      throw new Error(
        `Service "${params.name}:${params.version}" already exists`
      );
    }
    // Also check pending services
    if (pendingServices.value.some((s) => s.name === params.name && s.version === params.version)) {
      throw new Error(
        `Service "${params.name}:${params.version}" is already pending`
      );
    }

    // Resolve dependencies: convert (name, version) pairs to service_ids
    const dependencyIds: number[] = [];
    for (const [depName, depVersion] of params.dependencies) {
      const depId = findServiceId(depName, depVersion);
      if (depId === null) {
        throw new Error(
          `Dependency "${depName}:${depVersion}" not found. Please ensure the dependency service exists before adding this service.`
        );
      }
      dependencyIds.push(depId);
    }

    let serviceId: number;

    if (useMock.value) {
      // Mock mode: generate temporary ID
      serviceId = ++mockServiceIdCounter;
    } else {
      // Real mode: call backend API to get the real service ID
      // Backend will also resolve dependencies, but we need to resolve them here too
      // for pending services to have correct dependency_ids
      serviceId = await serviceApi.addService(params);
    }

    // Create the new service item with resolved dependencies
    const newService: ServiceItem = {
      service_id: serviceId,
      name: params.name,
      version: params.version,
      program: params.program,
      description: params.description ?? null,
      workspace: params.workspace ?? null,
      args: params.args,
      dependency_ids: dependencyIds, // Dependencies resolved to service_ids
      group_id: -1, // Indicates unassigned/pending
      status: "Stopped",
    };

    // Add to pending list
    pendingServices.value.push(newService);

    // Mark config as changed
    hasConfigChanges.value = true;

    return serviceId;
  }

  /**
   * Finds a service by name and version across all groups.
   * @param name - Service name.
   * @param version - Service version.
   * @returns The service and its group if found, null otherwise.
   */
  function findService(
    name: string,
    version: string
  ): { service: ServiceItem; group: GroupWithStatus } | null {
    for (const group of groups.value) {
      const service = group.services.find(
        (s) => s.name === name && s.version === version
      );
      if (service) {
        return { service, group };
      }
    }
    return null;
  }

  /**
   * Finds all services that depend on the given service.
   * Checks both groups and pending services.
   * @param serviceId - The service ID to check.
   * @returns Array of services that have this service as a dependency.
   */
  function findDependentServices(serviceId: number): ServiceItem[] {
    const dependents: ServiceItem[] = [];
    // Check services in groups
    for (const group of groups.value) {
      for (const service of group.services) {
        if (service.dependency_ids.includes(serviceId)) {
          dependents.push(service);
        }
      }
    }
    // Also check pending services (they may have dependencies resolved later)
    for (const service of pendingServices.value) {
      if (service.dependency_ids.includes(serviceId)) {
        dependents.push(service);
      }
    }
    return dependents;
  }

  /**
   * Removes a service by name and version.
   * In mock mode: removes from local state directly.
   * In real mode: calls the backend API and marks config as changed.
   * Prevents deletion if other services depend on it.
   * Also handles removal from pending services list.
   * @param params - Service name and version to identify the service.
   * @returns Promise resolving when the service is removed.
   * @throws Error if the service has dependents.
   */
  async function removeServiceFromStore(params: ServiceNameVersionParams): Promise<void> {
    // Check if service is in pending list first
    const pendingIndex = pendingServices.value.findIndex(
      (s) => s.name === params.name && s.version === params.version
    );
    
    if (pendingIndex !== -1) {
      // Service is pending - check for dependents before removing
      const pendingService = pendingServices.value[pendingIndex];
      const dependents = findDependentServices(pendingService.service_id);
      if (dependents.length > 0) {
        const depList = dependents.map((d) => `"${d.name}:${d.version}"`).join(", ");
        throw new Error(
          `Cannot delete "${params.name}:${params.version}" because it is required by: ${depList}`
        );
      }
      
      if (useMock.value) {
        // Mock mode: just remove from pending list
        pendingServices.value.splice(pendingIndex, 1);
        hasConfigChanges.value = true;
      } else {
        // Real mode: call backend API to delete from database, then remove from pending list
        await serviceApi.removeService(params);
        pendingServices.value.splice(pendingIndex, 1);
        hasConfigChanges.value = true;
      }
      return;
    }

    // Find the service in groups
    const result = findService(params.name, params.version);
    if (!result) {
      throw new Error(`Service "${params.name}:${params.version}" not found`);
    }

    const { service } = result;

    // Check for dependent services
    const dependents = findDependentServices(service.service_id);
    if (dependents.length > 0) {
      const depList = dependents.map((d) => `"${d.name}:${d.version}"`).join(", ");
      throw new Error(
        `Cannot delete "${params.name}:${params.version}" because it is required by: ${depList}`
      );
    }

    if (useMock.value) {
      // Remove from local state
      for (const group of groups.value) {
        const index = group.services.findIndex(
          (s) => s.name === params.name && s.version === params.version
        );
        if (index !== -1) {
          group.services.splice(index, 1);
          break;
        }
      }
      // Mark config as changed (for mock mode)
      hasConfigChanges.value = true;
      // No need to fetchGroups in mock mode since we updated state directly
    } else {
      // Real mode: call the backend API and refresh
      await serviceApi.removeService(params);
      hasConfigChanges.value = true;
      await fetchGroups();
    }
  }

  /**
   * Reloads the service configuration from the database.
   * In real mode: reloads configs from database, updates group membership, and refreshes the list.
   * In mock mode: clears the config changes flag (no-op for data).
   * @returns Resolves when the reload is complete.
   * @throws Rejects with an error message if the reload fails.
   */
  async function reloadServiceManager(): Promise<void> {
    if (useMock.value) {
      // In mock mode, just clear the flag and pending services
      hasConfigChanges.value = false;
      pendingServices.value = [];
      return;
    }
    // Real mode: sync with backend
    await serviceApi.reloadServiceManager();
    await serviceApi.updateServiceGroupMembership();
    hasConfigChanges.value = false;
    // Clear pending services after successful reload
    pendingServices.value = [];
    await fetchGroups();
  }

  return {
    groups,
    pendingServices,
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
    addServiceToStore,
    removeServiceFromStore,
    hasConfigChanges,
    reloadServiceManager,
  };
});
