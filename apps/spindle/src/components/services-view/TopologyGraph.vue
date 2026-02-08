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

/** Theme vars: borderColor follows theme (light/dark), use for border and shadow. */
const themeVars = useThemeVars();

const NODE_WIDTH = 220;
const NODE_HEIGHT = 96;

/** Dummy root id: nodes with no dependencies connect from this node. */
const DUMMY_START_ID = "__root";
/** Dummy sink id: leaf nodes (no downstream) connect to this node. */
const DUMMY_END_ID = "__sink";

/**
 * Unique key for a node; used to associate links with node positions.
 * @param data - Node datum (service or dummy).
 */
function nodeKey(data: ServiceItemType): string {
  if (data.name === DUMMY_START_ID) return DUMMY_START_ID;
  if (data.name === DUMMY_END_ID) return DUMMY_END_ID;
  return `${data.name}-${data.version}`;
}

/** Builds a dummy node datum for start/sink. */
function dummyDatum(name: string): ServiceItemType {
  return {
    service_id: -1,
    name,
    version: "",
    program: "",
    description: null,
    workspace: null,
    args: [],
    dependency_ids: [],
    group_id: -1,
    status: "Stopped",
  } as ServiceItemType;
}

const props = defineProps<{
  group: GroupWithStatus | null;
}>();

const emit = defineEmits<{
  (e: "node-click", service: ServiceItemType): void;
}>();

const containerRef = ref<HTMLElement | null>(null);
const graphContentRef = ref<HTMLElement | null>(null);

/** Container size; SVG/canvas matches parent; updated by ResizeObserver. */
const containerSize = ref({ width: 640, height: 480 });

const layoutResult = ref<{ width: number; height: number } | null>(null);
const nodes = ref<Array<{ x: number; y: number; data: ServiceItemType }>>([]);
/** Dummy start/end nodes (display only; no drag or click). */
const dummyNodes = ref<Array<{ x: number; y: number; kind: "start" | "end" }>>([]);
/** Pan offset from dragging empty area; used for node position and link path so links follow cards. */
const panOffset = ref({ x: 0, y: 0 });
/** Links: sourceKey/targetKey for drawing; points from layout (array or {x,y}). */
const links = ref<
  Array<{
    sourceKey: string;
    targetKey: string;
    points: [number, number][];
  }>
>([]);

/** Canvas size matches container. */
const canvasSize = computed(() => ({
  width: containerSize.value.width,
  height: containerSize.value.height,
}));

/** Precomputed path `d` for each link; avoids calling linkPathD per link on every render. */
const linkPathsD = computed(() =>
  links.value.map((link) =>
    linkPathD(link, nodes.value, dummyNodes.value, panOffset.value)
  )
);

/** Arrow position and rotation at each link midpoint; one-to-one with links. */
const linkArrows = computed(() =>
  links.value.map((link) =>
    linkArrowMid(link, nodes.value, dummyNodes.value, panOffset.value)
  )
);

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

let zoomBehavior: d3.ZoomBehavior<HTMLDivElement, unknown> | null = null;

/** Builds the DAG from group services and dependency_ids; returns null on empty or error. */
function buildDag() {
  const group = props.group;
  if (!group || group.services.length === 0) return null;

  const serviceMap = new Map(group.services.map((s) => [String(s.service_id), s]));
  const linkData: [string, string][] = [];

  for (const svc of group.services) {
    const targetId = String(svc.service_id);
    if (svc.dependency_ids.length > 0) {
      for (const depId of svc.dependency_ids) {
        linkData.push([String(depId), targetId]);
      }
    } else {
      linkData.push([DUMMY_START_ID, targetId]);
    }
  }

  const asSource = new Set(linkData.map(([s]) => s));
  for (const svc of group.services) {
    const id = String(svc.service_id);
    if (!asSource.has(id)) linkData.push([id, DUMMY_END_ID]);
  }

  const builder = graphConnect()
    .sourceId(([s]: [string, string]) => s)
    .targetId(([, t]: [string, string]) => t)
    .nodeDatum((id: string) => {
      if (id === DUMMY_START_ID) return dummyDatum(DUMMY_START_ID);
      if (id === DUMMY_END_ID) return dummyDatum(DUMMY_END_ID);
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
    const dag = builder(linkData as [string, string][]);
    return dag;
  } catch {
    return null;
  }
}

/** Runs DAG layout and updates nodes, dummyNodes, links, and layoutResult. */
function computeLayout() {
  const dag = buildDag();
  if (!dag) {
    layoutResult.value = null;
    nodes.value = [];
    dummyNodes.value = [];
    links.value = [];
    return;
  }

  const layout = sugiyama()
    .nodeSize([NODE_WIDTH, NODE_HEIGHT])
    .gap([128, 64])
    .coord(coordCenter())
    .decross(decrossTwoLayer().passes(20));

  const { width, height } = layout(dag);

  layoutResult.value = { width, height };
  panOffset.value = { x: 0, y: 0 };
  const nodeList = [...dag.nodes()];
  const isDummy = (name: string) => name === DUMMY_START_ID || name === DUMMY_END_ID;
  nodes.value = nodeList
    .filter((node) => !isDummy((node.data as ServiceItemType).name))
    .map((node) => ({
      x: node.x,
      y: node.y,
      data: node.data as ServiceItemType,
    }));
  dummyNodes.value = nodeList
    .filter((node) => isDummy((node.data as ServiceItemType).name))
    .map((node) => ({
      x: node.x,
      y: node.y,
      kind: (node.data as ServiceItemType).name === DUMMY_END_ID ? "end" : "start",
    }));
  links.value = [...dag.links()].map((link) => {
    const rawPoints = (link.points ?? []) as Array<[number, number] | { x: number; y: number }>;
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

type NodeItem = (typeof nodes.value)[number];
type DummyNodeItem = (typeof dummyNodes.value)[number];
type LinkItem = (typeof links.value)[number];

/**
 * Resolves screen position for a node key (real or dummy).
 * @param offset - Layout offset + pan offset.
 */
function resolveNodePosition(
  key: string,
  nodeList: NodeItem[],
  dummyList: DummyNodeItem[],
  offset: { x: number; y: number }
): { x: number; y: number } | null {
  if (key === DUMMY_START_ID) {
    const d = dummyList.find((n) => n.kind === "start");
    return d ? { x: d.x + offset.x, y: d.y + offset.y } : null;
  }
  if (key === DUMMY_END_ID) {
    const d = dummyList.find((n) => n.kind === "end");
    return d ? { x: d.x + offset.x, y: d.y + offset.y } : null;
  }
  const n = nodeList.find((node) => nodeKey(node.data) === key);
  return n ? { x: n.x + offset.x, y: n.y + offset.y } : null;
}

/**
 * Builds the path `d` for a link from current node/dummy positions and offset (same space as cards).
 */
function linkPathD(
  link: LinkItem,
  nodeList: NodeItem[],
  dummyList: DummyNodeItem[],
  offset: { x: number; y: number }
): string {
  const sourcePos = resolveNodePosition(link.sourceKey, nodeList, dummyList, offset);
  const targetPos = resolveNodePosition(link.targetKey, nodeList, dummyList, offset);
  if (!sourcePos || !targetPos) return "";

  const { x: sx, y: sy } = sourcePos;
  const { x: tx, y: ty } = targetPos;
  const midX = (sx + tx) / 2;
  return `M ${sx} ${sy} C ${midX} ${sy}, ${midX} ${ty}, ${tx} ${ty}`;
}

/** Cubic Bezier B(t) at t=0.5: (1-t)³P0 + 3(1-t)²t P1 + 3(1-t)t² P2 + t³ P3. */
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

/** Tangent direction (radians) of the Bezier at t=0.5, toward target. */
function bezierTangentAtMid(
  p0: { x: number; y: number },
  p1: { x: number; y: number },
  p2: { x: number; y: number },
  p3: { x: number; y: number }
): number {
  const t = 0.5;
  const dx = 3 * (1 - t) * (1 - t) * (p1.x - p0.x) + 6 * (1 - t) * t * (p2.x - p1.x) + 3 * t * t * (p3.x - p2.x);
  const dy = 3 * (1 - t) * (1 - t) * (p1.y - p0.y) + 6 * (1 - t) * t * (p2.y - p1.y) + 3 * t * t * (p3.y - p2.y);
  return Math.atan2(dy, dx);
}

/** Arrow position and rotation at link midpoint (same curve as linkPathD). */
function linkArrowMid(
  link: LinkItem,
  nodeList: NodeItem[],
  dummyList: DummyNodeItem[],
  offset: { x: number; y: number }
): { x: number; y: number; angleDeg: number } | null {
  const sourcePos = resolveNodePosition(link.sourceKey, nodeList, dummyList, offset);
  const targetPos = resolveNodePosition(link.targetKey, nodeList, dummyList, offset);
  if (!sourcePos || !targetPos) return null;

  const sx = sourcePos.x;
  const sy = sourcePos.y;
  const tx = targetPos.x;
  const ty = targetPos.y;
  const midX = (sx + tx) / 2;
  const p0 = { x: sx, y: sy };
  const p1 = { x: midX, y: sy };
  const p2 = { x: midX, y: ty };
  const p3 = { x: tx, y: ty };
  const mid = bezierMidpoint(p0, p1, p2, p3);
  const angleRad = bezierTangentAtMid(p0, p1, p2, p3);
  const angleDeg = (angleRad * 180) / Math.PI;
  return { x: mid.x, y: mid.y, angleDeg };
}

/** Binds d3 zoom (pan only) to container; drag on empty area updates panOffset. */
function setupZoom() {
  const container = containerRef.value;
  const graphContent = graphContentRef.value;
  if (!container || !graphContent || !layoutResult.value) return;

  const initialTransform = d3.zoomIdentity;

  zoomBehavior = d3
    .zoom<HTMLDivElement, unknown>()
    .scaleExtent([1, 1])
    .filter((event: MouseEvent) => {
      const target = event.target as Element;
      return !target.closest?.(".node-wrap") && !target.closest?.(".dummy-wrap");
    })
    .on("zoom", (event) => {
      panOffset.value = { x: event.transform.x, y: event.transform.y };
    });

  const sel = d3.select(container as HTMLDivElement);
  sel.call(zoomBehavior as d3.ZoomBehavior<HTMLDivElement, unknown>);
  sel.call(
    zoomBehavior.transform as (sel: d3.Selection<HTMLDivElement, unknown, null, undefined>, t: d3.ZoomTransform) => void,
    initialTransform
  );
}

function onNodeClick(service: ServiceItemType) {
  if (ignoreNextClick.value) return;
  emit("node-click", service);
}

let dragState: {
  nodeKey: string;
  startX: number;
  startY: number;
  startClientX: number;
  startClientY: number;
  moved: boolean;
} | null = null;
const ignoreNextClick = ref(false);

function onNodeMouseDown(e: MouseEvent, node: NodeItem) {
  if (e.button !== 0) return;
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

function onWindowMouseMove(e: MouseEvent) {
  if (!dragState) return;
  const dx = e.clientX - dragState.startClientX;
  const dy = e.clientY - dragState.startClientY;
  dragState.moved = true;
  nodes.value = nodes.value.map((n) =>
    nodeKey(n.data) === dragState!.nodeKey
      ? { ...n, x: dragState!.startX + dx, y: dragState!.startY + dy }
      : n
  );
}

function onWindowMouseUp() {
  if (dragState?.moved) {
    ignoreNextClick.value = true;
    setTimeout(() => {
      ignoreNextClick.value = false;
    }, 0);
  }
  dragState = null;
  window.removeEventListener("mousemove", onWindowMouseMove);
  window.removeEventListener("mouseup", onWindowMouseUp);
}

watch(
  () => props.group,
  () => computeLayout(),
  { deep: true }
);

onMounted(() => {
  computeLayout();
  const el = containerRef.value;
  if (el) {
    containerSize.value = { width: el.clientWidth, height: el.clientHeight };
    const ro = new ResizeObserver(() => {
      if (containerRef.value) {
        containerSize.value = {
          width: containerRef.value.clientWidth,
          height: containerRef.value.clientHeight,
        };
      }
    });
    ro.observe(el);
    onUnmounted(() => ro.disconnect());
  }
});

watch([layoutResult, containerRef, graphContentRef, containerSize], () => {
  if (layoutResult.value && containerRef.value && graphContentRef.value && containerSize.value.width > 0) {
    setTimeout(setupZoom, 0);
  }
});

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
            <ServiceItem :service="node.data" layout="card" @click="onNodeClick(node.data)" />
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
