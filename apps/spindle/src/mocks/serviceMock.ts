/**
 * Mock service data for demo and development.
 *
 * @remarks
 * To disable: delete this file and in `serviceStore.ts` use `[]` or an inline fallback instead of
 * `mockGroupsForDev` / `mockGroups`.
 *
 * Rules: (1) If a dependency is not Running (Stopped/Error), dependents cannot be Running.
 * (2) Max services per group is 16.
 */
import type { GroupWithStatus, ServiceItem } from "@/types/service.types";

/** Maximum number of services allowed in a single mock group. */
export const MAX_SERVICES_PER_GROUP = 16;

/**
 * Build a {@link ServiceItem} with defaults and overrides.
 * @param overrides - At least name, version, and status; other fields optional.
 * @returns A full ServiceItem.
 */
function svc(overrides: Partial<ServiceItem> & Pick<ServiceItem, "name" | "version" | "status">): ServiceItem {
  return {
    service_id: 0,
    program: "node",
    description: null,
    workspace: null,
    args: [],
    dependency_ids: [],
    group_id: 0,
    ...overrides,
  };
}

/**
 * Validates that no Running service has a dependency that is not Running.
 * @param services - List of services (must be indexable by service_id).
 * @returns `true` if the dependency rule holds.
 */
export function validateDependencyRule(services: ServiceItem[]): boolean {
  const byId = new Map(services.map((s) => [s.service_id, s]));
  for (const s of services) {
    if (s.status !== "Running") continue;
    for (const depId of s.dependency_ids) {
      const dep = byId.get(depId);
      if (dep && dep.status !== "Running") return false;
    }
  }
  return true;
}

/** Returns a new array of services sorted by name (localeCompare). */
function sortByName(services: ServiceItem[]): ServiceItem[] {
  return [...services].sort((a, b) => a.name.localeCompare(b.name));
}

// ---------------------------------------------------------------------------
// Default showcase groups (satisfy dependency rule)
// ---------------------------------------------------------------------------

/**
 * Default showcase mock groups (four groups). Remove this export and the import in
 * serviceStore to disable mock.
 */
export const mockGroups: GroupWithStatus[] = [
  {
    group_id: 0,
    alias: "API Stack",
    displayName: "API Stack",
    services: sortByName([
      svc({ service_id: 1, name: "gateway", version: "1.0.0", status: "Running", dependency_ids: [2, 3], group_id: 0 }),
      svc({ service_id: 2, name: "auth", version: "1.0.0", status: "Running", dependency_ids: [], group_id: 0 }),
      svc({ service_id: 3, name: "api", version: "2.1.0", status: "Running", dependency_ids: [4], group_id: 0 }),
      svc({ service_id: 4, name: "db-proxy", version: "1.0.0", status: "Running", dependency_ids: [], group_id: 0 }),
    ]),
  },
  {
    group_id: 1,
    alias: "Worker Pool",
    displayName: "Worker Pool",
    services: sortByName([
      svc({ service_id: 5, name: "worker-a", version: "1.0.0", status: "Running", dependency_ids: [7], group_id: 1 }),
      svc({ service_id: 6, name: "worker-b", version: "1.0.0", status: "Stopped", dependency_ids: [7], group_id: 1 }),
      svc({ service_id: 7, name: "queue", version: "1.0.0", status: "Running", dependency_ids: [], group_id: 1 }),
    ]),
  },
  {
    group_id: 2,
    alias: "Data Pipeline",
    displayName: "Data Pipeline",
    services: sortByName([
      svc({ service_id: 8, name: "ingest", version: "1.0.0", status: "Stopped", dependency_ids: [9], group_id: 2 }),
      svc({ service_id: 9, name: "processor", version: "1.0.0", status: "Stopped", dependency_ids: [10], group_id: 2 }),
      svc({ service_id: 10, name: "sink", version: "1.0.0", status: "Stopped", dependency_ids: [], group_id: 2 }),
    ]),
  },
  {
    group_id: 3,
    alias: null,
    displayName: "Unnamed Group 0",
    services: [
      svc({ service_id: 11, name: "scheduler", version: "2.0.0", status: "Stopped", dependency_ids: [], group_id: 3 }),
    ],
  },
];

// ---------------------------------------------------------------------------
// Topology scenario groups: for layout preview only. Group names stay abstract (e.g. Chain, Fan-out).
// Node names use common service names for display. Dependency rule enforced; max 16 per group.
// ---------------------------------------------------------------------------

const v = "1.0.0";
const g0 = 0;

/** Builds a topology scenario group with the given alias and services. */
function topo(alias: string, services: ServiceItem[]): GroupWithStatus {
  return {
    group_id: 0,
    alias,
    displayName: alias,
    services: sortByName(services),
  };
}

/** Chain (3): gateway -> api -> db, all Running */
const scenarioChain: GroupWithStatus[] = [
  topo("Chain (3)", [
    svc({ service_id: 1, name: "gateway", version: v, status: "Running", dependency_ids: [2], group_id: g0 }),
    svc({ service_id: 2, name: "api", version: v, status: "Running", dependency_ids: [3], group_id: g0 }),
    svc({ service_id: 3, name: "db", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
  ]),
];

/** Chain all Stopped */
const scenarioChainStopped: GroupWithStatus[] = [
  topo("Chain Stopped (3)", [
    svc({ service_id: 1, name: "gateway", version: v, status: "Stopped", dependency_ids: [2], group_id: g0 }),
    svc({ service_id: 2, name: "api", version: v, status: "Stopped", dependency_ids: [3], group_id: g0 }),
    svc({ service_id: 3, name: "db", version: v, status: "Stopped", dependency_ids: [], group_id: g0 }),
  ]),
];

/** Fan-out (5): gateway -> auth, api, cache, redis, queue */
const scenarioFanOut: GroupWithStatus[] = [
  topo("Fan-out (5)", [
    svc({ service_id: 1, name: "gateway", version: v, status: "Running", dependency_ids: [2, 3, 4, 5], group_id: g0 }),
    svc({ service_id: 2, name: "auth", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 3, name: "api", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 4, name: "cache", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 5, name: "redis", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
  ]),
];

/** Fan-in (4): ingest, processor, worker -> sink */
const scenarioFanIn: GroupWithStatus[] = [
  topo("Fan-in (4)", [
    svc({ service_id: 1, name: "ingest", version: v, status: "Running", dependency_ids: [4], group_id: g0 }),
    svc({ service_id: 2, name: "processor", version: v, status: "Running", dependency_ids: [4], group_id: g0 }),
    svc({ service_id: 3, name: "worker", version: v, status: "Running", dependency_ids: [4], group_id: g0 }),
    svc({ service_id: 4, name: "sink", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
  ]),
];

/** Diamond (4): gateway -> api, auth -> db */
const scenarioDiamond: GroupWithStatus[] = [
  topo("Diamond (4)", [
    svc({ service_id: 1, name: "gateway", version: v, status: "Running", dependency_ids: [2, 3], group_id: g0 }),
    svc({ service_id: 2, name: "api", version: v, status: "Running", dependency_ids: [4], group_id: g0 }),
    svc({ service_id: 3, name: "auth", version: v, status: "Running", dependency_ids: [4], group_id: g0 }),
    svc({ service_id: 4, name: "db", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
  ]),
];

/** Hourglass (5): gateway -> api, cache, proxy -> db */
const scenarioHourglass: GroupWithStatus[] = [
  topo("Hourglass (5)", [
    svc({ service_id: 1, name: "gateway", version: v, status: "Running", dependency_ids: [2, 3, 4], group_id: g0 }),
    svc({ service_id: 2, name: "api", version: v, status: "Running", dependency_ids: [5], group_id: g0 }),
    svc({ service_id: 3, name: "cache", version: v, status: "Running", dependency_ids: [5], group_id: g0 }),
    svc({ service_id: 4, name: "proxy", version: v, status: "Running", dependency_ids: [5], group_id: g0 }),
    svc({ service_id: 5, name: "db", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
  ]),
];

/** Two tiers (6): queue -> worker-1..worker-5 */
const scenarioTwoTiers: GroupWithStatus[] = [
  topo("Two Tiers (6)", [
    svc({ service_id: 1, name: "queue", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 2, name: "worker-1", version: v, status: "Running", dependency_ids: [1], group_id: g0 }),
    svc({ service_id: 3, name: "worker-2", version: v, status: "Running", dependency_ids: [1], group_id: g0 }),
    svc({ service_id: 4, name: "worker-3", version: v, status: "Running", dependency_ids: [1], group_id: g0 }),
    svc({ service_id: 5, name: "worker-4", version: v, status: "Running", dependency_ids: [1], group_id: g0 }),
    svc({ service_id: 6, name: "worker-5", version: v, status: "Running", dependency_ids: [1], group_id: g0 }),
  ]),
];

/** Two chains (6): gateway -> api->db and gateway -> worker->sink */
const scenarioTwoChains: GroupWithStatus[] = [
  topo("Two Chains (6)", [
    svc({ service_id: 1, name: "gateway", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 2, name: "api", version: v, status: "Running", dependency_ids: [1], group_id: g0 }),
    svc({ service_id: 3, name: "db", version: v, status: "Running", dependency_ids: [2], group_id: g0 }),
    svc({ service_id: 4, name: "worker", version: v, status: "Running", dependency_ids: [1], group_id: g0 }),
    svc({ service_id: 5, name: "queue", version: v, status: "Running", dependency_ids: [4], group_id: g0 }),
    svc({ service_id: 6, name: "sink", version: v, status: "Running", dependency_ids: [5], group_id: g0 }),
  ]),
];

/** Layered bipartite (5): auth, api, cache -> aggregator, processor */
const scenarioLayeredBipartite: GroupWithStatus[] = [
  topo("Layered Bipartite (5)", [
    svc({ service_id: 1, name: "auth", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 2, name: "api", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 3, name: "cache", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 4, name: "aggregator", version: v, status: "Running", dependency_ids: [1, 2], group_id: g0 }),
    svc({ service_id: 5, name: "processor", version: v, status: "Running", dependency_ids: [2, 3], group_id: g0 }),
  ]),
];

/** Binary tree (7): gateway -> api, auth -> db-1..db-4 */
const scenarioBinaryTree: GroupWithStatus[] = [
  topo("Binary Tree (7)", [
    svc({ service_id: 1, name: "gateway", version: v, status: "Running", dependency_ids: [2, 3], group_id: g0 }),
    svc({ service_id: 2, name: "api", version: v, status: "Running", dependency_ids: [4, 5], group_id: g0 }),
    svc({ service_id: 3, name: "auth", version: v, status: "Running", dependency_ids: [6, 7], group_id: g0 }),
    svc({ service_id: 4, name: "db-1", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 5, name: "db-2", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 6, name: "redis-1", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 7, name: "redis-2", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
  ]),
];

/** Deep chain (6): gateway -> api -> cache -> db -> worker -> sink */
const scenarioDeepChain: GroupWithStatus[] = [
  topo("Deep Chain (6)", [
    svc({ service_id: 1, name: "gateway", version: v, status: "Running", dependency_ids: [2], group_id: g0 }),
    svc({ service_id: 2, name: "api", version: v, status: "Running", dependency_ids: [3], group_id: g0 }),
    svc({ service_id: 3, name: "cache", version: v, status: "Running", dependency_ids: [4], group_id: g0 }),
    svc({ service_id: 4, name: "db", version: v, status: "Running", dependency_ids: [5], group_id: g0 }),
    svc({ service_id: 5, name: "worker", version: v, status: "Running", dependency_ids: [6], group_id: g0 }),
    svc({ service_id: 6, name: "sink", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
  ]),
];

/** Wide fan-out (9): gateway -> 8 downstream services */
const scenarioWideFanOut: GroupWithStatus[] = [
  topo("Wide Fan-out (9)", [
    svc({ service_id: 1, name: "gateway", version: v, status: "Running", dependency_ids: [2, 3, 4, 5, 6, 7, 8, 9], group_id: g0 }),
    svc({ service_id: 2, name: "auth", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 3, name: "api", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 4, name: "cache", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 5, name: "redis", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 6, name: "queue", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 7, name: "worker", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 8, name: "processor", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 9, name: "sink", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
  ]),
];

/** Max tree (16): gateway -> 5 mid -> 10 leaves */
const scenarioMaxTree: GroupWithStatus[] = [
  topo("Max Tree (16)", [
    svc({ service_id: 1, name: "gateway", version: v, status: "Running", dependency_ids: [2, 3, 4, 5, 6], group_id: g0 }),
    svc({ service_id: 2, name: "api-a", version: v, status: "Running", dependency_ids: [7, 8], group_id: g0 }),
    svc({ service_id: 3, name: "api-b", version: v, status: "Running", dependency_ids: [9, 10], group_id: g0 }),
    svc({ service_id: 4, name: "api-c", version: v, status: "Running", dependency_ids: [11, 12], group_id: g0 }),
    svc({ service_id: 5, name: "api-d", version: v, status: "Running", dependency_ids: [13, 14], group_id: g0 }),
    svc({ service_id: 6, name: "api-e", version: v, status: "Running", dependency_ids: [15, 16], group_id: g0 }),
    svc({ service_id: 7, name: "db-1", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 8, name: "db-2", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 9, name: "redis-1", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 10, name: "redis-2", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 11, name: "cache-1", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 12, name: "cache-2", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 13, name: "queue-1", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 14, name: "queue-2", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 15, name: "worker-1", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 16, name: "worker-2", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
  ]),
];

/** Partial stopped: fan-out, some leaves Stopped so root cannot be Running */
const scenarioPartialStopped: GroupWithStatus[] = [
  topo("Partial Stopped (5)", [
    svc({ service_id: 1, name: "gateway", version: v, status: "Stopped", dependency_ids: [2, 3, 4, 5], group_id: g0 }),
    svc({ service_id: 2, name: "auth", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 3, name: "api", version: v, status: "Stopped", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 4, name: "cache", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
    svc({ service_id: 5, name: "redis", version: v, status: "Stopped", dependency_ids: [], group_id: g0 }),
  ]),
];

/** Error in middle: client -> broker(Error) -> db, client Stopped */
const scenarioErrorInMiddle: GroupWithStatus[] = [
  topo("Error in Middle (3)", [
    svc({ service_id: 1, name: "client", version: v, status: "Stopped", dependency_ids: [2], group_id: g0 }),
    svc({ service_id: 2, name: "broker", version: v, status: "Error", dependency_ids: [3], group_id: g0 }),
    svc({ service_id: 3, name: "db", version: v, status: "Running", dependency_ids: [], group_id: g0 }),
  ]),
];

/** Single node */
const scenarioSingle: GroupWithStatus[] = [
  topo("Single Node (1)", [
    svc({ service_id: 1, name: "scheduler", version: v, status: "Stopped", dependency_ids: [], group_id: g0 }),
  ]),
];

// ---------------------------------------------------------------------------
// Topology-only scenario groups for layout preview (no business mock groups)
// ---------------------------------------------------------------------------

/** Clones a group and assigns the given group_id to the group and all its services. */
function withGroupId(g: GroupWithStatus, groupId: number): GroupWithStatus {
  return {
    ...g,
    group_id: groupId,
    services: g.services.map((s) => ({ ...s, group_id: groupId })),
  };
}

const topologyScenarios: GroupWithStatus[][] = [
  scenarioChain,
  scenarioChainStopped,
  scenarioFanOut,
  scenarioFanIn,
  scenarioDiamond,
  scenarioHourglass,
  scenarioTwoTiers,
  scenarioTwoChains,
  scenarioLayeredBipartite,
  scenarioBinaryTree,
  scenarioDeepChain,
  scenarioWideFanOut,
  scenarioMaxTree,
  scenarioPartialStopped,
  scenarioErrorInMiddle,
  scenarioSingle,
];

/**
 * All topology scenario groups with unique group_ids; used when USE_MOCK is true for layout preview.
 */
export const mockGroupsForDev: GroupWithStatus[] = topologyScenarios.flatMap((groups, idx) =>
  groups.map((g) => withGroupId(g, idx))
);
