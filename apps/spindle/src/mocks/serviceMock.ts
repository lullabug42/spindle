/**
 * Mock service data for quantum computing laboratory equipment drivers.
 *
 * @remarks
 * This mock data simulates equipment drivers for various quantum computing approaches
 * (superconducting circuits, trapped ions, neutral atoms, etc.) in a research lab.
 *
 * Database Constraints Compliance:
 * - All service_ids are globally unique (PRIMARY KEY)
 * - (name, version) combinations are UNIQUE across all services
 * - dependency_ids reference existing services (FOREIGN KEY constraint)
 * - No self-references in dependencies (CHECK service_id != dependency_id)
 * - Each service belongs to exactly one group (per service_group_membership)
 * - group_id is consistent between service and group
 */
import type { GroupWithStatus, ServiceItem } from "@/types/service.types";

/** Maximum number of services allowed in a single mock group. */
export const MAX_SERVICES_PER_GROUP = 16;

/** Counter for generating unique service IDs across all mock data. */
let globalServiceIdCounter = 100;

/** Set to track used (name, version) combinations for uniqueness. */
const usedNameVersions = new Set<string>();

/** Generates a new globally unique service ID. */
function nextId(): number {
  return ++globalServiceIdCounter;
}

/**
 * Build a {@link ServiceItem} with defaults and overrides.
 * Validates (name, version) uniqueness.
 * @param overrides - At least name, version, and status; other fields optional.
 * @returns A full ServiceItem.
 * @throws Error if (name, version) combination already exists.
 */
function svc(overrides: Partial<ServiceItem> & Pick<ServiceItem, "name" | "version" | "status">): ServiceItem {
  const key = `${overrides.name}:${overrides.version}`;
  if (usedNameVersions.has(key)) {
    throw new Error(`Duplicate (name, version) combination: ${key}`);
  }
  usedNameVersions.add(key);

  return {
    service_id: nextId(),
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
 * Also validates that all dependency_ids exist and are not self-references.
 * @param services - List of services (must be indexable by service_id).
 * @returns `true` if all validation rules hold.
 */
export function validateDependencyRule(services: ServiceItem[]): boolean {
  const byId = new Map(services.map((s) => [s.service_id, s]));

  for (const s of services) {
    // Validate dependency_ids exist and are not self-references
    for (const depId of s.dependency_ids) {
      // Check for self-reference (CHECK constraint: service_id != dependency_id)
      if (s.service_id === depId) {
        console.error(`Self-reference detected: service ${s.name}:${s.version} (id=${s.service_id}) depends on itself`);
        return false;
      }
      // Check foreign key constraint (dependency must exist)
      if (!byId.has(depId)) {
        console.error(`Foreign key violation: service ${s.name}:${s.version} depends on non-existent service_id=${depId}`);
        return false;
      }
    }

    // Check Running dependency rule
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
// Default showcase groups for quantum computing lab
// Service IDs: 101-111
// ---------------------------------------------------------------------------

/**
 * Default showcase mock groups for quantum computing laboratory.
 * Equipment groups: Cryogenic System, Qubit Control, Readout Chain, Timing System
 */
export const mockGroups: GroupWithStatus[] = [
  {
    group_id: 0,
    alias: "Cryogenic System",
    displayName: "Cryogenic System",
    services: sortByName([
      svc({ name: "cryo-controller", version: "3.2.1", status: "Running" }),
      svc({ name: "temp-monitor", version: "2.0.0", status: "Running" }),
      svc({ name: "magnet-controller", version: "1.5.0", status: "Running" }),
      svc({ name: "vacuum-pump", version: "1.0.0", status: "Running" }),
    ]).map((s) => ({ ...s, group_id: 0 })),
  },
  {
    group_id: 1,
    alias: "Qubit Control",
    displayName: "Qubit Control",
    services: sortByName([
      svc({ name: "qubit-control-a", version: "2.1.0", status: "Running" }),
      svc({ name: "qubit-control-b", version: "2.1.0", status: "Stopped" }),
      svc({ name: "mw-source", version: "4.0.0", status: "Running" }),
    ]).map((s) => ({ ...s, group_id: 1 })),
  },
  {
    group_id: 2,
    alias: "Readout Chain",
    displayName: "Readout Chain",
    services: sortByName([
      svc({ name: "readout-amp", version: "1.8.0", status: "Stopped" }),
      svc({ name: "digitizer", version: "2.3.0", status: "Stopped" }),
      svc({ name: "data-recorder", version: "1.0.0", status: "Stopped" }),
    ]).map((s) => ({ ...s, group_id: 2 })),
  },
  {
    group_id: 3,
    alias: null,
    displayName: "Unnamed Group 0",
    services: [
      svc({ name: "sync-trigger", version: "1.0.0", status: "Stopped", group_id: 3 }),
    ],
  },
];

// Fix dependencies for showcase groups using actual generated IDs
// Group 0: Cryogenic System - cryo-controller depends on temp-monitor and magnet-controller; magnet-controller depends on vacuum-pump
(function fixGroup0() {
  const group = mockGroups[0];
  const cryo = group.services.find(s => s.name === "cryo-controller")!;
  const temp = group.services.find(s => s.name === "temp-monitor")!;
  const magnet = group.services.find(s => s.name === "magnet-controller")!;
  const vacuum = group.services.find(s => s.name === "vacuum-pump")!;
  cryo.dependency_ids = [temp.service_id, magnet.service_id];
  magnet.dependency_ids = [vacuum.service_id];
})();

// Group 1: Qubit Control - qubit controllers depend on mw-source
(function fixGroup1() {
  const group = mockGroups[1];
  const qubitA = group.services.find(s => s.name === "qubit-control-a")!;
  const qubitB = group.services.find(s => s.name === "qubit-control-b")!;
  const mw = group.services.find(s => s.name === "mw-source")!;
  qubitA.dependency_ids = [mw.service_id];
  qubitB.dependency_ids = [mw.service_id];
})();

// Group 2: Readout Chain - readout-amp -> digitizer -> data-recorder
(function fixGroup2() {
  const group = mockGroups[2];
  const amp = group.services.find(s => s.name === "readout-amp")!;
  const dig = group.services.find(s => s.name === "digitizer")!;
  const recorder = group.services.find(s => s.name === "data-recorder")!;
  amp.dependency_ids = [dig.service_id];
  dig.dependency_ids = [recorder.service_id];
})();

// Validate showcase groups
for (const group of mockGroups) {
  if (!validateDependencyRule(group.services)) {
    console.error(`Validation failed for group: ${group.displayName}`);
  }
}

// Reset counter for topology scenarios (ensure no ID overlap)
// Start from 200 to ensure clear separation from showcase groups (101-111)
globalServiceIdCounter = 200;

// Clear name-version tracking for topology scenarios (each scenario is independent)
usedNameVersions.clear();

// ---------------------------------------------------------------------------
// Topology scenario groups for quantum lab equipment
// Service IDs start from 201 to ensure global uniqueness.
// Each scenario uses sequential IDs within its range.
// ---------------------------------------------------------------------------

// Each topology scenario uses unique version numbers to avoid (name, version) conflicts
// when scenarios are combined in mockGroupsForDev.
// Versions follow realistic SemVer patterns with alpha, beta, rc suffixes.
const V_CHAIN = "1.2.3";
const V_CHAIN_STOPPED = "1.2.3-beta.1";
const V_FAN_OUT = "2.1.0-alpha.2";
const V_FAN_IN = "1.5.2-rc.1";
const V_DIAMOND = "3.0.1";
const V_HOURGLASS = "2.3.0-beta.2";
const V_TWO_TIERS = "1.8.5";
const V_TWO_CHAINS = "2.0.0-rc.2";
const V_LAYERED = "1.9.0-alpha.3";
const V_BINARY_TREE = "2.5.1";
const V_DEEP_CHAIN = "1.7.3-beta.3";
const V_WIDE_FAN_OUT = "3.1.0-rc.3";
const V_MAX_TREE = "2.4.2";
const V_PARTIAL_STOPPED = "1.6.0-alpha.4";
const V_ERROR_MIDDLE = "2.2.0-beta.4";
const V_SINGLE = "1.0.1";

/** Builds a topology scenario group with the given alias and services. */
function topo(alias: string, services: ServiceItem[]): GroupWithStatus {
  return {
    group_id: 0,
    alias,
    displayName: alias,
    // Note: services are NOT sorted here to preserve creation order for dependency setup
    // Sorting will be done after dependencies are set
    services,
  };
}

/** Helper to find a service by name in a group's services array. */
function findService(services: ServiceItem[], name: string): ServiceItem | undefined {
  return services.find(s => s.name === name);
}

/** Chain (3): signal-generator -> mixer -> amplifier */
const scenarioChain: GroupWithStatus[] = [
  topo("Chain (3)", [
    svc({ name: "signal-generator", version: V_CHAIN, status: "Running" }),
    svc({ name: "mixer", version: V_CHAIN, status: "Running" }),
    svc({ name: "amplifier", version: V_CHAIN, status: "Running" }),
  ]),
];
// Set dependencies by name to avoid index issues
(function setupChain() {
  const services = scenarioChain[0].services;
  const sg = findService(services, "signal-generator")!;
  const mixer = findService(services, "mixer")!;
  const amp = findService(services, "amplifier")!;
  sg.dependency_ids = [mixer.service_id];
  mixer.dependency_ids = [amp.service_id];
  // Sort after dependencies are set
  services.sort((a, b) => a.name.localeCompare(b.name));
})();

/** Chain all Stopped */
const scenarioChainStopped: GroupWithStatus[] = [
  topo("Chain Stopped (3)", [
    svc({ name: "signal-generator", version: V_CHAIN_STOPPED, status: "Stopped" }),
    svc({ name: "mixer", version: V_CHAIN_STOPPED, status: "Stopped" }),
    svc({ name: "amplifier", version: V_CHAIN_STOPPED, status: "Stopped" }),
  ]),
];
(function setupChainStopped() {
  const services = scenarioChainStopped[0].services;
  const sg = findService(services, "signal-generator")!;
  const mixer = findService(services, "mixer")!;
  const amp = findService(services, "amplifier")!;
  sg.dependency_ids = [mixer.service_id];
  mixer.dependency_ids = [amp.service_id];
  services.sort((a, b) => a.name.localeCompare(b.name));
})();

/** Fan-out (5): mw-source -> qubit-1,2,3,4 (microwave distribution) */
const scenarioFanOut: GroupWithStatus[] = [
  topo("Fan-out (5)", [
    svc({ name: "mw-source", version: V_FAN_OUT, status: "Running" }),
    svc({ name: "qubit-1", version: V_FAN_OUT, status: "Running" }),
    svc({ name: "qubit-2", version: V_FAN_OUT, status: "Running" }),
    svc({ name: "qubit-3", version: V_FAN_OUT, status: "Running" }),
    svc({ name: "qubit-4", version: V_FAN_OUT, status: "Running" }),
  ]),
];
(function setupFanOut() {
  const services = scenarioFanOut[0].services;
  const mw = findService(services, "mw-source")!;
  const qubits = ["qubit-1", "qubit-2", "qubit-3", "qubit-4"]
    .map(name => findService(services, name)!.service_id);
  mw.dependency_ids = qubits;
  services.sort((a, b) => a.name.localeCompare(b.name));
})();

/** Fan-in (4): readout-1,2,3 -> readout-amp (readout multiplexing) */
const scenarioFanIn: GroupWithStatus[] = [
  topo("Fan-in (4)", [
    svc({ name: "readout-1", version: V_FAN_IN, status: "Running" }),
    svc({ name: "readout-2", version: V_FAN_IN, status: "Running" }),
    svc({ name: "readout-3", version: V_FAN_IN, status: "Running" }),
    svc({ name: "readout-amp", version: V_FAN_IN, status: "Running" }),
  ]),
];
(function setupFanIn() {
  const services = scenarioFanIn[0].services;
  const readouts = ["readout-1", "readout-2", "readout-3"]
    .map(name => findService(services, name)!);
  const amp = findService(services, "readout-amp")!;
  readouts.forEach(r => r.dependency_ids = [amp.service_id]);
  services.sort((a, b) => a.name.localeCompare(b.name));
})();

/** Diamond (4): cryostat -> magnet, temp-ctrl -> sample-stage */
const scenarioDiamond: GroupWithStatus[] = [
  topo("Diamond (4)", [
    svc({ name: "cryostat", version: V_DIAMOND, status: "Running" }),
    svc({ name: "magnet", version: V_DIAMOND, status: "Running" }),
    svc({ name: "temp-ctrl", version: V_DIAMOND, status: "Running" }),
    svc({ name: "sample-stage", version: V_DIAMOND, status: "Running" }),
  ]),
];
(function setupDiamond() {
  const services = scenarioDiamond[0].services;
  const cryo = findService(services, "cryostat")!;
  const magnet = findService(services, "magnet")!;
  const temp = findService(services, "temp-ctrl")!;
  const sample = findService(services, "sample-stage")!;
  cryo.dependency_ids = [magnet.service_id, temp.service_id];
  magnet.dependency_ids = [sample.service_id];
  temp.dependency_ids = [sample.service_id];
  services.sort((a, b) => a.name.localeCompare(b.name));
})();

/** Hourglass (5): controller -> pulse-gen, wg-gen, awg -> sample */
const scenarioHourglass: GroupWithStatus[] = [
  topo("Hourglass (5)", [
    svc({ name: "controller", version: V_HOURGLASS, status: "Running" }),
    svc({ name: "pulse-gen", version: V_HOURGLASS, status: "Running" }),
    svc({ name: "wg-gen", version: V_HOURGLASS, status: "Running" }),
    svc({ name: "awg", version: V_HOURGLASS, status: "Running" }),
    svc({ name: "sample", version: V_HOURGLASS, status: "Running" }),
  ]),
];
(function setupHourglass() {
  const services = scenarioHourglass[0].services;
  const ctrl = findService(services, "controller")!;
  const pulse = findService(services, "pulse-gen")!;
  const wg = findService(services, "wg-gen")!;
  const awg = findService(services, "awg")!;
  const sample = findService(services, "sample")!;
  ctrl.dependency_ids = [pulse.service_id, wg.service_id, awg.service_id];
  pulse.dependency_ids = [sample.service_id];
  wg.dependency_ids = [sample.service_id];
  awg.dependency_ids = [sample.service_id];
  services.sort((a, b) => a.name.localeCompare(b.name));
})();

/** Two tiers (6): voltage-source -> dc-1..dc-5 (DC bias lines) */
const scenarioTwoTiers: GroupWithStatus[] = [
  topo("Two Tiers (6)", [
    svc({ name: "voltage-source", version: V_TWO_TIERS, status: "Running" }),
    svc({ name: "dc-1", version: V_TWO_TIERS, status: "Running" }),
    svc({ name: "dc-2", version: V_TWO_TIERS, status: "Running" }),
    svc({ name: "dc-3", version: V_TWO_TIERS, status: "Running" }),
    svc({ name: "dc-4", version: V_TWO_TIERS, status: "Running" }),
    svc({ name: "dc-5", version: V_TWO_TIERS, status: "Running" }),
  ]),
];
(function setupTwoTiers() {
  const services = scenarioTwoTiers[0].services;
  const voltage = findService(services, "voltage-source")!;
  ["dc-1", "dc-2", "dc-3", "dc-4", "dc-5"]
    .forEach(name => {
      const dc = findService(services, name)!;
      dc.dependency_ids = [voltage.service_id];
    });
  services.sort((a, b) => a.name.localeCompare(b.name));
})();

/** Two chains (6): sync -> sg->amp and sync -> li->detector */
const scenarioTwoChains: GroupWithStatus[] = [
  topo("Two Chains (6)", [
    svc({ name: "sync", version: V_TWO_CHAINS, status: "Running" }),
    svc({ name: "sg", version: V_TWO_CHAINS, status: "Running" }),
    svc({ name: "amp", version: V_TWO_CHAINS, status: "Running" }),
    svc({ name: "lia", version: V_TWO_CHAINS, status: "Running" }),
    svc({ name: "detector", version: V_TWO_CHAINS, status: "Running" }),
    svc({ name: "analyzer", version: V_TWO_CHAINS, status: "Running" }),
  ]),
];
(function setupTwoChains() {
  const services = scenarioTwoChains[0].services;
  const sync = findService(services, "sync")!;
  const sg = findService(services, "sg")!;
  const amp = findService(services, "amp")!;
  const lia = findService(services, "lia")!;
  const detector = findService(services, "detector")!;
  const analyzer = findService(services, "analyzer")!;
  // Chain 1: sync -> sg -> amp
  sg.dependency_ids = [sync.service_id];
  amp.dependency_ids = [sg.service_id];
  // Chain 2: sync -> lia -> detector -> analyzer
  lia.dependency_ids = [sync.service_id];
  detector.dependency_ids = [lia.service_id];
  analyzer.dependency_ids = [detector.service_id];
  services.sort((a, b) => a.name.localeCompare(b.name));
})();

/** Layered bipartite (5): na, sa, ps -> qctrl-a, qctrl-b (multi-instrument control) */
const scenarioLayeredBipartite: GroupWithStatus[] = [
  topo("Layered Bipartite (5)", [
    svc({ name: "na", version: V_LAYERED, status: "Running" }),
    svc({ name: "sa", version: V_LAYERED, status: "Running" }),
    svc({ name: "ps", version: V_LAYERED, status: "Running" }),
    svc({ name: "qctrl-a", version: V_LAYERED, status: "Running" }),
    svc({ name: "qctrl-b", version: V_LAYERED, status: "Running" }),
  ]),
];
(function setupLayered() {
  const services = scenarioLayeredBipartite[0].services;
  const na = findService(services, "na")!;
  const sa = findService(services, "sa")!;
  const ps = findService(services, "ps")!;
  const qctrlA = findService(services, "qctrl-a")!;
  const qctrlB = findService(services, "qctrl-b")!;
  qctrlA.dependency_ids = [na.service_id, sa.service_id];
  qctrlB.dependency_ids = [sa.service_id, ps.service_id];
  services.sort((a, b) => a.name.localeCompare(b.name));
})();

/** Binary tree (7): master -> sg1,sg2 -> hemt1..hemt4 (hierarchical signal gen) */
const scenarioBinaryTree: GroupWithStatus[] = [
  topo("Binary Tree (7)", [
    svc({ name: "master", version: V_BINARY_TREE, status: "Running" }),
    svc({ name: "sg1", version: V_BINARY_TREE, status: "Running" }),
    svc({ name: "sg2", version: V_BINARY_TREE, status: "Running" }),
    svc({ name: "hemt1", version: V_BINARY_TREE, status: "Running" }),
    svc({ name: "hemt2", version: V_BINARY_TREE, status: "Running" }),
    svc({ name: "hemt3", version: V_BINARY_TREE, status: "Running" }),
    svc({ name: "hemt4", version: V_BINARY_TREE, status: "Running" }),
  ]),
];
(function setupBinaryTree() {
  const services = scenarioBinaryTree[0].services;
  const master = findService(services, "master")!;
  const sg1 = findService(services, "sg1")!;
  const sg2 = findService(services, "sg2")!;
  const hemt1 = findService(services, "hemt1")!;
  const hemt2 = findService(services, "hemt2")!;
  const hemt3 = findService(services, "hemt3")!;
  const hemt4 = findService(services, "hemt4")!;
  master.dependency_ids = [sg1.service_id, sg2.service_id];
  sg1.dependency_ids = [hemt1.service_id, hemt2.service_id];
  sg2.dependency_ids = [hemt3.service_id, hemt4.service_id];
  services.sort((a, b) => a.name.localeCompare(b.name));
})();

/** Deep chain (6): cryo->magnet->sample->probe->amp->daq (full measurement chain) */
const scenarioDeepChain: GroupWithStatus[] = [
  topo("Deep Chain (6)", [
    svc({ name: "cryo", version: V_DEEP_CHAIN, status: "Running" }),
    svc({ name: "magnet", version: V_DEEP_CHAIN, status: "Running" }),
    svc({ name: "sample", version: V_DEEP_CHAIN, status: "Running" }),
    svc({ name: "probe", version: V_DEEP_CHAIN, status: "Running" }),
    svc({ name: "amp", version: V_DEEP_CHAIN, status: "Running" }),
    svc({ name: "daq", version: V_DEEP_CHAIN, status: "Running" }),
  ]),
];
(function setupDeepChain() {
  const services = scenarioDeepChain[0].services;
  const chain = ["cryo", "magnet", "sample", "probe", "amp", "daq"]
    .map(name => findService(services, name)!);
  for (let i = 0; i < chain.length - 1; i++) {
    chain[i].dependency_ids = [chain[i + 1].service_id];
  }
  services.sort((a, b) => a.name.localeCompare(b.name));
})();

/** Wide fan-out (9): master-clock -> 8 instrument sync lines */
const scenarioWideFanOut: GroupWithStatus[] = [
  topo("Wide Fan-out (9)", [
    svc({ name: "master-clock", version: V_WIDE_FAN_OUT, status: "Running" }),
    svc({ name: "sync-1", version: V_WIDE_FAN_OUT, status: "Running" }),
    svc({ name: "sync-2", version: V_WIDE_FAN_OUT, status: "Running" }),
    svc({ name: "sync-3", version: V_WIDE_FAN_OUT, status: "Running" }),
    svc({ name: "sync-4", version: V_WIDE_FAN_OUT, status: "Running" }),
    svc({ name: "sync-5", version: V_WIDE_FAN_OUT, status: "Running" }),
    svc({ name: "sync-6", version: V_WIDE_FAN_OUT, status: "Running" }),
    svc({ name: "sync-7", version: V_WIDE_FAN_OUT, status: "Running" }),
    svc({ name: "sync-8", version: V_WIDE_FAN_OUT, status: "Running" }),
  ]),
];
(function setupWideFanOut() {
  const services = scenarioWideFanOut[0].services;
  const master = findService(services, "master-clock")!;
  const syncs = [1, 2, 3, 4, 5, 6, 7, 8]
    .map(i => findService(services, `sync-${i}`)!.service_id);
  master.dependency_ids = syncs;
  services.sort((a, b) => a.name.localeCompare(b.name));
})();

/** Max tree (16): master -> 5 instrument groups -> 10 devices */
const scenarioMaxTree: GroupWithStatus[] = [
  topo("Max Tree (16)", [
    svc({ name: "master", version: V_MAX_TREE, status: "Running" }),
    svc({ name: "instr-a", version: V_MAX_TREE, status: "Running" }),
    svc({ name: "instr-b", version: V_MAX_TREE, status: "Running" }),
    svc({ name: "instr-c", version: V_MAX_TREE, status: "Running" }),
    svc({ name: "instr-d", version: V_MAX_TREE, status: "Running" }),
    svc({ name: "instr-e", version: V_MAX_TREE, status: "Running" }),
    svc({ name: "dev-1", version: V_MAX_TREE, status: "Running" }),
    svc({ name: "dev-2", version: V_MAX_TREE, status: "Running" }),
    svc({ name: "dev-3", version: V_MAX_TREE, status: "Running" }),
    svc({ name: "dev-4", version: V_MAX_TREE, status: "Running" }),
    svc({ name: "dev-5", version: V_MAX_TREE, status: "Running" }),
    svc({ name: "dev-6", version: V_MAX_TREE, status: "Running" }),
    svc({ name: "dev-7", version: V_MAX_TREE, status: "Running" }),
    svc({ name: "dev-8", version: V_MAX_TREE, status: "Running" }),
    svc({ name: "dev-9", version: V_MAX_TREE, status: "Running" }),
    svc({ name: "dev-10", version: V_MAX_TREE, status: "Running" }),
  ]),
];
(function setupMaxTree() {
  const services = scenarioMaxTree[0].services;
  const master = findService(services, "master")!;
  const instrA = findService(services, "instr-a")!;
  const instrB = findService(services, "instr-b")!;
  const instrC = findService(services, "instr-c")!;
  const instrD = findService(services, "instr-d")!;
  const instrE = findService(services, "instr-e")!;
  const devs = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    .map(i => findService(services, `dev-${i}`)!);
  // master -> instr-a..instr-e
  master.dependency_ids = [instrA, instrB, instrC, instrD, instrE].map(s => s.service_id);
  // instr-a -> dev-1, dev-2
  instrA.dependency_ids = [devs[0].service_id, devs[1].service_id];
  // instr-b -> dev-3, dev-4
  instrB.dependency_ids = [devs[2].service_id, devs[3].service_id];
  // instr-c -> dev-5, dev-6
  instrC.dependency_ids = [devs[4].service_id, devs[5].service_id];
  // instr-d -> dev-7, dev-8
  instrD.dependency_ids = [devs[6].service_id, devs[7].service_id];
  // instr-e -> dev-9, dev-10
  instrE.dependency_ids = [devs[8].service_id, devs[9].service_id];
  services.sort((a, b) => a.name.localeCompare(b.name));
})();

/** Partial stopped: some instruments stopped, master cannot run */
const scenarioPartialStopped: GroupWithStatus[] = [
  topo("Partial Stopped (5)", [
    svc({ name: "master-clock", version: V_PARTIAL_STOPPED, status: "Stopped" }),
    svc({ name: "sg-1", version: V_PARTIAL_STOPPED, status: "Running" }),
    svc({ name: "sg-2", version: V_PARTIAL_STOPPED, status: "Stopped" }),
    svc({ name: "sg-3", version: V_PARTIAL_STOPPED, status: "Running" }),
    svc({ name: "sg-4", version: V_PARTIAL_STOPPED, status: "Stopped" }),
  ]),
];
(function setupPartialStopped() {
  const services = scenarioPartialStopped[0].services;
  const master = findService(services, "master-clock")!;
  const sgs = [1, 2, 3, 4].map(i => findService(services, `sg-${i}`)!.service_id);
  master.dependency_ids = sgs;
  services.sort((a, b) => a.name.localeCompare(b.name));
})();

/** Error in middle: trigger -> pulse-gen(Error) -> qubit */
const scenarioErrorInMiddle: GroupWithStatus[] = [
  topo("Error in Middle (3)", [
    svc({ name: "trigger", version: V_ERROR_MIDDLE, status: "Stopped" }),
    svc({ name: "pulse-gen", version: V_ERROR_MIDDLE, status: "Error" }),
    svc({ name: "qubit", version: V_ERROR_MIDDLE, status: "Running" }),
  ]),
];
(function setupErrorInMiddle() {
  const services = scenarioErrorInMiddle[0].services;
  const trigger = findService(services, "trigger")!;
  const pulse = findService(services, "pulse-gen")!;
  const qubit = findService(services, "qubit")!;
  trigger.dependency_ids = [pulse.service_id];
  pulse.dependency_ids = [qubit.service_id];
  services.sort((a, b) => a.name.localeCompare(b.name));
})();

/** Single node */
const scenarioSingle: GroupWithStatus[] = [
  topo("Single Node (1)", [
    svc({ name: "oscilloscope", version: V_SINGLE, status: "Stopped" }),
  ]),
];
(function setupSingle() {
  scenarioSingle[0].services.sort((a, b) => a.name.localeCompare(b.name));
})();

// Validate all topology scenarios individually
const topologyScenariosList = [
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

for (const scenario of topologyScenariosList) {
  for (const group of scenario) {
    if (!validateDependencyRule(group.services)) {
      console.error(`Validation failed for scenario: ${group.displayName}`);
    }
  }
}

// ---------------------------------------------------------------------------
// Topology-only scenario groups for layout preview
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
 * All topology scenario groups with unique group_ids and globally unique service_ids;
 * used when USE_MOCK is true for layout preview.
 */
export const mockGroupsForDev: GroupWithStatus[] = topologyScenarios.flatMap((groups, idx) =>
  groups.map((g) => withGroupId(g, idx))
);

// Final validation for mockGroupsForDev
const allMockGroupsForDevServices = mockGroupsForDev.flatMap(g => g.services);
if (!validateDependencyRule(allMockGroupsForDevServices)) {
  console.error("Validation failed for mockGroupsForDev");
}
