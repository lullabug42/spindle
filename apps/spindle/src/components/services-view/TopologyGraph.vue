<!--
  TopologyGraph: DAG layout of services and dependencies with pan and node drag.
  Uses d3-dag sugiyama layout; dummy start/end nodes for roots and leaves.
-->
<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from "vue";
import { useThemeVars } from "naive-ui";
import { graphConnect, sugiyama, coordCenter, decrossTwoLayer } from "d3-dag";
import * as d3 from "d3";
import ServiceItem from "./ServiceItem.vue";
import DummyNode from "./DummyNode.vue";
import type { GroupWithStatus, ServiceItem as ServiceItemType } from "@/types/service.types";

// ============================================================================
// Constants
// ============================================================================

/** Theme vars: borderColor follows theme (light/dark), use for border and shadow. */
const themeVars = useThemeVars();

/** Node dimensions for layout calculation. */
const NODE_WIDTH = 220;
const NODE_HEIGHT = 96;

/** Dummy root id: nodes with no dependencies connect from this node. */
const DUMMY_START_ID = "__root";
/** Dummy sink id: leaf nodes (no downstream) connect to this node. */
const DUMMY_END_ID = "__sink";

// ============================================================================
// Type Definitions
// ============================================================================

type NodeItem = { x: number; y: number; data: ServiceItemType };
type DummyNodeItem = { x: number; y: number; kind: "start" | "end" };
type LinkItem = {
  sourceKey: string;
  targetKey: string;
  points: [number, number][];
};

// ============================================================================
// Utility Functions
// ============================================================================

/**
 * Generates a unique key for a node; used to associate links with node positions.
 * @param data - Node datum (service or dummy).
 */
function nodeKey(data: ServiceItemType): string {
  if (data.name === DUMMY_START_ID) return DUMMY_START_ID;
  if (data.name === DUMMY_END_ID) return DUMMY_END_ID;
  return `${data.name}-${data.version}`;
}

/**
 * Checks if a node is a dummy node (start or end).
 */
function isDummyNode(name: string): boolean {
  return name === DUMMY_START_ID || name === DUMMY_END_ID;
}

/**
 * Builds a dummy node datum for start/sink nodes.
 * @param name - Either DUMMY_START_ID or DUMMY_END_ID.
 */
function createDummyDatum(name: string): ServiceItemType {
  return {
    service_id: -1,
    name,
    version: "",
    program: "",
    description: null,
    workspace: null,
    args: [],
    dependency_ids: [],
    group_id: null,
    status: "Stopped",
  } as ServiceItemType;
}

// ============================================================================
// Component Props & Emits
// ============================================================================

const props = defineProps<{
  group: GroupWithStatus | null;
}>();

const emit = defineEmits<{
  (e: "node-click", service: ServiceItemType): void;
}>();

// ============================================================================
// Refs & State
// ============================================================================

const containerRef = ref<HTMLElement | null>(null);
const graphContentRef = ref<HTMLElement | null>(null);

/** Container size; SVG/canvas matches parent; updated by ResizeObserver. */
const containerSize = ref({ width: 640, height: 480 });

/** Layout calculation result (width and height of the DAG). */
const layoutResult = ref<{ width: number; height: number } | null>(null);

/** Real service nodes with their positions. */
const nodes = ref<NodeItem[]>([]);

/** Dummy start/end nodes (display only; no drag or click). */
const dummyNodes = ref<DummyNodeItem[]>([]);

/** Pan offset from dragging empty area; used for node position and link path. */
const panOffset = ref({ x: 0, y: 0 });

/** Links between nodes: sourceKey/targetKey for drawing; points from layout. */
const links = ref<LinkItem[]>([]);

/** Flag to ignore next click event (used after drag to prevent accidental clicks). */
const ignoreNextClick = ref(false);

/** D3 zoom behavior instance. */
let zoomBehavior: d3.ZoomBehavior<HTMLDivElement, unknown> | null = null;

/** Drag state for node dragging. */
let dragState: {
  nodeKey: string;
  startX: number;
  startY: number;
  startClientX: number;
  startClientY: number;
  moved: boolean;
} | null = null;

// ============================================================================
// Computed Properties
// ============================================================================

/** Canvas size matches container. */
const canvasSize = computed(() => ({
  width: containerSize.value.width,
  height: containerSize.value.height,
}));

/** Offset to center the layout inside the canvas. */
const layoutOffset = computed(() => {
  const layout = layoutResult.value;
  const canvas = canvasSize.value;
  if (!layout) return { x: 0, y: 0 };
  return {
    x: (canvas.width - layout.width) / 2,
    y: (canvas.height - layout.height) / 2,
  };
});

/** Precomputed SVG path `d` attribute for each link. */
const linkPathsD = computed(() =>
  links.value.map((link) =>
    linkPathD(link, nodes.value, dummyNodes.value, panOffset.value)
  )
);

/** Arrow position and rotation at each link midpoint. */
const linkArrows = computed(() =>
  links.value.map((link) =>
    linkArrowMid(link, nodes.value, dummyNodes.value, panOffset.value)
  )
);

// ============================================================================
// DAG Construction
// ============================================================================

/**
 * Builds the DAG from group services and dependency_ids.
 * Adds dummy start/end nodes for roots and leaves.
 * @returns The constructed DAG, or null if group is empty or construction fails.
 */
function buildDag() {
  const group = props.group;
  if (!group || group.services.length === 0) return null;

  // Create a map for quick service lookup by ID
  const serviceMap = new Map(
    group.services.map((s) => [String(s.service_id), s])
  );

  // Build link data: [sourceId, targetId] pairs
  const linkData: [string, string][] = [];

  // Add links from dependencies to services
  for (const service of group.services) {
    const targetId = String(service.service_id);
    if (service.dependency_ids.length > 0) {
      // Service has dependencies: link from each dependency to this service
      for (const depId of service.dependency_ids) {
        linkData.push([String(depId), targetId]);
      }
    } else {
      // Service has no dependencies: link from dummy start node
      linkData.push([DUMMY_START_ID, targetId]);
    }
  }

  // Find leaf nodes (services that are not sources of any link) and link to dummy end
  const sourceIds = new Set(linkData.map(([sourceId]) => sourceId));
  for (const service of group.services) {
    const serviceId = String(service.service_id);
    if (!sourceIds.has(serviceId)) {
      linkData.push([serviceId, DUMMY_END_ID]);
    }
  }

  // Build the DAG using d3-dag
  const builder = graphConnect()
    .sourceId(([sourceId]: [string, string]) => sourceId)
    .targetId(([, targetId]: [string, string]) => targetId)
    .nodeDatum((id: string) => {
      // Return dummy node for start/end, or actual service
      if (id === DUMMY_START_ID) return createDummyDatum(DUMMY_START_ID);
      if (id === DUMMY_END_ID) return createDummyDatum(DUMMY_END_ID);
      return (
        serviceMap.get(id) ??
        ({
          service_id: 0,
          name: id,
          version: "",
          program: "",
          description: null,
          workspace: null,
          args: [],
          dependency_ids: [],
          group_id: 0,
          status: "Stopped",
        } as ServiceItemType)
      );
    })
    .single(true);

  try {
    return builder(linkData as [string, string][]);
  } catch {
    return null;
  }
}

// ============================================================================
// Layout Computation
// ============================================================================

/**
 * Runs DAG layout calculation and updates nodes, dummyNodes, links, and layoutResult.
 * Uses sugiyama layout algorithm from d3-dag.
 */
function computeLayout() {
  const dag = buildDag();
  if (!dag) {
    // Clear all layout data if DAG construction failed
    layoutResult.value = null;
    nodes.value = [];
    dummyNodes.value = [];
    links.value = [];
    return;
  }

  // Configure sugiyama layout
  const layout = sugiyama()
    .nodeSize([NODE_WIDTH, NODE_HEIGHT])
    .gap([128, 128])
    .coord(coordCenter())
    .decross(decrossTwoLayer().passes(20));

  // Calculate layout
  const { width, height } = layout(dag);
  layoutResult.value = { width, height };
  panOffset.value = { x: 0, y: 0 }; // Reset pan on layout recalculation

  // Extract nodes from DAG
  const nodeList = [...dag.nodes()];

  // Separate real nodes from dummy nodes
  nodes.value = nodeList
    .filter((node) => !isDummyNode((node.data as ServiceItemType).name))
    .map((node) => ({
      x: node.x,
      y: node.y,
      data: node.data as ServiceItemType,
    }));

  dummyNodes.value = nodeList
    .filter((node) => isDummyNode((node.data as ServiceItemType).name))
    .map((node) => {
      const name = (node.data as ServiceItemType).name;
      return {
        x: node.x,
        y: node.y,
        kind: name === DUMMY_END_ID ? "end" : "start",
      };
    });

  // Extract links and normalize point coordinates
  links.value = [...dag.links()].map((link) => {
    const rawPoints = (link.points ?? []) as Array<
      [number, number] | { x: number; y: number }
    >;
    const points: [number, number][] = rawPoints.map((p) =>
      Array.isArray(p) ? [p[0], p[1]] : [p.x, p.y]
    );
    return {
      sourceKey: nodeKey(link.source.data as ServiceItemType),
      targetKey: nodeKey(link.target.data as ServiceItemType),
      points,
    };
  });
}

// ============================================================================
// Position Resolution
// ============================================================================

/**
 * Resolves screen position for a node by its key (real or dummy).
 * @param key - Node key (from nodeKey function or DUMMY_START_ID/DUMMY_END_ID).
 * @param nodeList - List of real service nodes.
 * @param dummyList - List of dummy nodes.
 * @param offset - Combined layout offset + pan offset.
 * @returns Screen position {x, y} or null if node not found.
 */
function resolveNodePosition(
  key: string,
  nodeList: NodeItem[],
  dummyList: DummyNodeItem[],
  offset: { x: number; y: number }
): { x: number; y: number } | null {
  if (key === DUMMY_START_ID) {
    const dummy = dummyList.find((n) => n.kind === "start");
    return dummy ? { x: dummy.x + offset.x, y: dummy.y + offset.y } : null;
  }
  if (key === DUMMY_END_ID) {
    const dummy = dummyList.find((n) => n.kind === "end");
    return dummy ? { x: dummy.x + offset.x, y: dummy.y + offset.y } : null;
  }
  const node = nodeList.find((node) => nodeKey(node.data) === key);
  return node ? { x: node.x + offset.x, y: node.y + offset.y } : null;
}

// ============================================================================
// Link Rendering
// ============================================================================

/**
 * Builds the SVG path `d` attribute for a link (cubic Bezier curve).
 * @param link - Link item with sourceKey and targetKey.
 * @param nodeList - List of real service nodes.
 * @param dummyList - List of dummy nodes.
 * @param offset - Combined layout offset + pan offset.
 * @returns SVG path string or empty string if positions not found.
 */
function linkPathD(
  link: LinkItem,
  nodeList: NodeItem[],
  dummyList: DummyNodeItem[],
  offset: { x: number; y: number }
): string {
  const sourcePos = resolveNodePosition(
    link.sourceKey,
    nodeList,
    dummyList,
    offset
  );
  const targetPos = resolveNodePosition(
    link.targetKey,
    nodeList,
    dummyList,
    offset
  );
  if (!sourcePos || !targetPos) return "";

  // Create a smooth cubic Bezier curve
  const { x: sx, y: sy } = sourcePos;
  const { x: tx, y: ty } = targetPos;
  const midX = (sx + tx) / 2;
  return `M ${sx} ${sy} C ${midX} ${sy}, ${midX} ${ty}, ${tx} ${ty}`;
}

/**
 * Calculates the midpoint of a cubic Bezier curve at t=0.5.
 * Formula: B(t) = (1-t)³P0 + 3(1-t)²t P1 + 3(1-t)t² P2 + t³ P3
 */
function bezierMidpoint(
  p0: { x: number; y: number },
  p1: { x: number; y: number },
  p2: { x: number; y: number },
  p3: { x: number; y: number }
): { x: number; y: number } {
  const t = 0.5;
  const u = 1 - t;
  const u3 = u * u * u;
  const u2t = u * u * t * 3;
  const ut2 = u * t * t * 3;
  const t3 = t * t * t;
  return {
    x: u3 * p0.x + u2t * p1.x + ut2 * p2.x + t3 * p3.x,
    y: u3 * p0.y + u2t * p1.y + ut2 * p2.y + t3 * p3.y,
  };
}

/**
 * Calculates the tangent direction (in radians) of a cubic Bezier curve at t=0.5.
 * Used to orient arrows along the curve direction.
 */
function bezierTangentAtMid(
  p0: { x: number; y: number },
  p1: { x: number; y: number },
  p2: { x: number; y: number },
  p3: { x: number; y: number }
): number {
  const t = 0.5;
  const dx =
    3 * (1 - t) * (1 - t) * (p1.x - p0.x) +
    6 * (1 - t) * t * (p2.x - p1.x) +
    3 * t * t * (p3.x - p2.x);
  const dy =
    3 * (1 - t) * (1 - t) * (p1.y - p0.y) +
    6 * (1 - t) * t * (p2.y - p1.y) +
    3 * t * t * (p3.y - p2.y);
  return Math.atan2(dy, dx);
}

/**
 * Calculates arrow position and rotation at link midpoint.
 * Uses the same Bezier curve as linkPathD to ensure arrow aligns with the curve.
 * @returns Arrow position {x, y} and rotation angle in degrees, or null if positions not found.
 */
function linkArrowMid(
  link: LinkItem,
  nodeList: NodeItem[],
  dummyList: DummyNodeItem[],
  offset: { x: number; y: number }
): { x: number; y: number; angleDeg: number } | null {
  const sourcePos = resolveNodePosition(
    link.sourceKey,
    nodeList,
    dummyList,
    offset
  );
  const targetPos = resolveNodePosition(
    link.targetKey,
    nodeList,
    dummyList,
    offset
  );
  if (!sourcePos || !targetPos) return null;

  // Build Bezier control points (same as linkPathD)
  const { x: sx, y: sy } = sourcePos;
  const { x: tx, y: ty } = targetPos;
  const midX = (sx + tx) / 2;
  const p0 = { x: sx, y: sy };
  const p1 = { x: midX, y: sy };
  const p2 = { x: midX, y: ty };
  const p3 = { x: tx, y: ty };

  // Calculate midpoint and tangent direction
  const mid = bezierMidpoint(p0, p1, p2, p3);
  const angleRad = bezierTangentAtMid(p0, p1, p2, p3);
  const angleDeg = (angleRad * 180) / Math.PI;

  return { x: mid.x, y: mid.y, angleDeg };
}

// ============================================================================
// Interaction Handlers
// ============================================================================

/**
 * Sets up d3 zoom behavior for panning (zoom disabled, scale fixed at 1).
 * Panning only works on empty areas (not on nodes).
 */
function setupZoom() {
  const container = containerRef.value;
  const graphContent = graphContentRef.value;
  if (!container || !graphContent || !layoutResult.value) return;

  zoomBehavior = d3
    .zoom<HTMLDivElement, unknown>()
    .scaleExtent([1, 1]) // Disable zooming, only allow panning
    .filter((event: MouseEvent) => {
      // Only allow panning on empty areas, not on nodes
      const target = event.target as Element;
      return (
        !target.closest?.(".node-wrap") && !target.closest?.(".dummy-wrap")
      );
    })
    .on("zoom", (event) => {
      panOffset.value = { x: event.transform.x, y: event.transform.y };
    });

  const selection = d3.select(container as HTMLDivElement);
  selection.call(zoomBehavior as d3.ZoomBehavior<HTMLDivElement, unknown>);
  selection.call(
    zoomBehavior.transform as (
      sel: d3.Selection<HTMLDivElement, unknown, null, undefined>,
      t: d3.ZoomTransform
    ) => void,
    d3.zoomIdentity
  );
}

/**
 * Handles node click events.
 * Ignores clicks that occur immediately after a drag operation.
 */
function onNodeClick(service: ServiceItemType) {
  if (ignoreNextClick.value) return;
  emit("node-click", service);
}

/**
 * Handles node drag start (mousedown on a node).
 * Only responds to left mouse button (button 0).
 */
function onNodeMouseDown(e: MouseEvent, node: NodeItem) {
  if (e.button !== 0) return; // Only left mouse button
  e.preventDefault();

  dragState = {
    nodeKey: nodeKey(node.data),
    startX: node.x,
    startY: node.y,
    startClientX: e.clientX,
    startClientY: e.clientY,
    moved: false,
  };

  window.addEventListener("mousemove", onWindowMouseMove);
  window.addEventListener("mouseup", onWindowMouseUp);
}

/**
 * Handles node dragging (mousemove while dragging).
 * Updates the dragged node's position.
 */
function onWindowMouseMove(e: MouseEvent) {
  if (!dragState) return;

  const dx = e.clientX - dragState.startClientX;
  const dy = e.clientY - dragState.startClientY;
  dragState.moved = true;

  // Update the dragged node's position
  nodes.value = nodes.value.map((n) =>
    nodeKey(n.data) === dragState!.nodeKey
      ? { ...n, x: dragState!.startX + dx, y: dragState!.startY + dy }
      : n
  );
}

/**
 * Handles node drag end (mouseup).
 * If the node was moved, ignore the next click to prevent accidental navigation.
 */
function onWindowMouseUp() {
  if (dragState?.moved) {
    // Ignore click event immediately after drag
    ignoreNextClick.value = true;
    setTimeout(() => {
      ignoreNextClick.value = false;
    }, 0);
  }

  dragState = null;
  window.removeEventListener("mousemove", onWindowMouseMove);
  window.removeEventListener("mouseup", onWindowMouseUp);
}

// ============================================================================
// Lifecycle & Watchers
// ============================================================================

// Recompute layout when group changes
watch(
  () => props.group,
  () => computeLayout(),
  { deep: true }
);

onMounted(() => {
  computeLayout();

  // Set up ResizeObserver to track container size changes
  const container = containerRef.value;
  if (container) {
    containerSize.value = {
      width: container.clientWidth,
      height: container.clientHeight,
    };

    const resizeObserver = new ResizeObserver(() => {
      if (containerRef.value) {
        containerSize.value = {
          width: containerRef.value.clientWidth,
          height: containerRef.value.clientHeight,
        };
      }
    });
    resizeObserver.observe(container);
    onUnmounted(() => resizeObserver.disconnect());
  }
});

// Set up zoom behavior when layout is ready
watch(
  [layoutResult, containerRef, graphContentRef, containerSize],
  () => {
    if (
      layoutResult.value &&
      containerRef.value &&
      graphContentRef.value &&
      containerSize.value.width > 0
    ) {
      // Use setTimeout to ensure DOM is ready
      setTimeout(setupZoom, 0);
    }
  }
);

onUnmounted(() => {
  zoomBehavior = null;
});
</script>

<template>
  <div ref="containerRef" class="topology-container" :style="{
    '--topology-shadow-color': themeVars.borderColor,
    border: `1px solid ${themeVars.borderColor}`,
  }">
    <template v-if="layoutResult && nodes.length">
      <div ref="graphContentRef" class="graph-content" :style="{
        width: canvasSize.width + 'px',
        height: canvasSize.height + 'px',
      }">
        <svg class="topology-svg" :width="canvasSize.width" :height="canvasSize.height">
          <g class="links" :transform="`translate(${layoutOffset.x}, ${layoutOffset.y})`">
            <path v-for="(link, i) in links" :key="`${link.sourceKey}-${link.targetKey}`" :d="linkPathsD[i]"
              class="link-path" fill="none" stroke-width="2" />
            <g v-for="(arrow, i) in linkArrows" :key="`arrow-${links[i].sourceKey}-${links[i].targetKey}`"
              v-show="arrow" :transform="arrow ? `translate(${arrow.x}, ${arrow.y}) rotate(${arrow.angleDeg})` : ''">
              <polygon points="0,0 -8,-5 -8,5" class="link-arrow" />
            </g>
          </g>
        </svg>
        <div class="nodes-overlay">
          <div v-for="(dummy, i) in dummyNodes" :key="`dummy-${dummy.kind}-${i}`" class="dummy-wrap" :style="{
            left: layoutOffset.x + dummy.x + panOffset.x + 'px',
            top: layoutOffset.y + dummy.y + panOffset.y + 'px',
            width: NODE_WIDTH + 'px',
            height: NODE_HEIGHT + 'px',
            marginLeft: -NODE_WIDTH / 2 + 'px',
            marginTop: -NODE_HEIGHT / 2 + 'px',
          }">
            <DummyNode :kind="dummy.kind" />
          </div>
          <div v-for="node in nodes" :key="`${node.data.name}-${node.data.version}`" class="node-wrap" :style="{
            left: layoutOffset.x + node.x + panOffset.x + 'px',
            top: layoutOffset.y + node.y + panOffset.y + 'px',
            width: NODE_WIDTH + 'px',
            height: NODE_HEIGHT + 'px',
            marginLeft: -NODE_WIDTH / 2 + 'px',
            marginTop: -NODE_HEIGHT / 2 + 'px',
          }" @mousedown="onNodeMouseDown($event, node)" @click="onNodeClick(node.data)">
            <ServiceItem :service="node.data" layout="card" />
          </div>
        </div>
      </div>
    </template>
    <div v-else class="topology-empty">No services or no dependencies to display.</div>
  </div>
</template>

<style scoped>
/* --topology-shadow-color set via :style from themeVars.borderColor (follows light/dark). */
.topology-container {
  position: relative;
  width: 90%;
  height: 90%;
  min-width: 640px;
  min-height: 640px;
  margin: 0 auto;
  overflow: hidden;
  border-radius: 8px;
  background: var(--n-color-modal);
  box-shadow: 0 4px 24px color-mix(in srgb, var(--topology-shadow-color) 25%, transparent);
}

/* Must be auto so clicks hit nodes and drag works. */
.graph-content {
  position: absolute;
  left: 0;
  top: 0;
  transform-origin: 0 0;
  pointer-events: auto;
  z-index: 1;
  cursor: grab;
}

.graph-content:active {
  cursor: grabbing;
}

.graph-content .nodes-overlay {
  pointer-events: none;
}

.graph-content .node-wrap,
.graph-content .dummy-wrap {
  pointer-events: auto;
}

.graph-content .node-wrap {
  cursor: grab;
}

.graph-content .node-wrap:active {
  cursor: grabbing;
}

.topology-svg {
  position: absolute;
  left: 0;
  top: 0;
  display: block;
  pointer-events: none;
}

.nodes-overlay {
  position: absolute;
  left: 0;
  top: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
}

.node-wrap,
.dummy-wrap {
  position: absolute;
  box-sizing: border-box;
}

.node-wrap {
  cursor: pointer;
}

.node-wrap :deep(.service-item-card),
.node-wrap :deep(.n-card),
.dummy-wrap :deep(.n-card) {
  width: 100%;
  height: 100%;
  min-height: 96px;
  overflow: hidden;
}

.node-wrap :deep(.n-card__content),
.dummy-wrap :deep(.n-card__content) {
  overflow: hidden;
}

.topology-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 200px;
  color: var(--n-text-color-3);
}

/* Visible in light/dark: neutral gray, semi-transparent. */
.link-path {
  pointer-events: none;
  stroke: #64748b;
  stroke-opacity: 0.85;
}

/* Arrow at link midpoint; same color as link. */
.link-arrow {
  pointer-events: none;
  fill: #64748b;
  fill-opacity: 0.85;
}
</style>
