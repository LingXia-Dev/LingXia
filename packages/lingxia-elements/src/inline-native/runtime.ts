import { buildRootCommit, type NativeRootCommitJson } from "./commit.js";
import { buildGeometrySnapshot, type NativeGeometrySnapshotJson } from "./geometry.js";
import { identifyCompiledRoot, type IdentifiedRoot } from "./identity.js";
import {
  emptyViewLease,
  viewAcceptLease,
  viewApplyGrant,
  viewCanShowFallback,
  viewMarkActive,
  type ViewLeaseState,
} from "./lease.js";
import type { CompiledNativeRoot, RootRef } from "./types.js";

export interface RootRuntimeState {
  identified: IdentifiedRoot | null;
  treeRevision: number;
  geometryRevision: number;
  lease: ViewLeaseState;
}

export interface RootHostMessages {
  commit?: NativeRootCommitJson;
  geometry: NativeGeometrySnapshotJson;
  leaseAccept?: {
    action: "root.leaseAccept";
    id: string;
    root: RootRef;
    leaseId: string;
    sequence: number;
  };
  ready: boolean;
}

export function createRootRuntimeState(): RootRuntimeState {
  return {
    identified: null,
    treeRevision: 0,
    geometryRevision: 0,
    lease: emptyViewLease(),
  };
}

export function publishCompiledRoot(options: {
  compiled: CompiledNativeRoot;
  rootRef: RootRef;
  state: RootRuntimeState;
  rootRect: { x: number; y: number; width: number; height: number };
  nodeRects: Record<string, { x: number; y: number; width: number; height: number }>;
  nodeVisibility?: Record<string, boolean>;
  nodeClipStacks?: Record<string, unknown[]>;
  rootVisible?: boolean;
  rootOrder?: number;
  viewportOffset?: { x: number; y: number };
  nowMs?: number;
}): { state: RootRuntimeState; messages: RootHostMessages } {
  const identified = identifyCompiledRoot(
    options.compiled,
    options.rootRef,
    options.state.identified
  );
  const first = options.state.identified === null;
  let treeRevision = options.state.treeRevision;
  let commit: NativeRootCommitJson | undefined;
  if (first) {
    treeRevision = 1;
    commit = buildRootCommit(identified, null, 1);
  } else {
    const next = buildRootCommit(
      identified,
      options.state.identified,
      options.state.treeRevision + 1
    );
    if (next.operations.length > 0) {
      treeRevision = options.state.treeRevision + 1;
      commit = next;
    }
  }
  const geometryRevision = options.state.geometryRevision + 1;
  const nodeRects = mapRectsToNodeKeys(identified, options.nodeRects);
  const nodeVisibility = mapValuesToNodeKeys(identified, options.nodeVisibility ?? {});
  const nodeClipStacks = mapValuesToNodeKeys(identified, options.nodeClipStacks ?? {});
  const geometry = buildGeometrySnapshot({
    identified,
    basisTreeRevision: Math.max(treeRevision, 1),
    geometryRevision,
    rootOrder: options.rootOrder ?? 0,
    rootRect: options.rootRect,
    nodeRects,
    nodeVisibility,
    nodeClipStacks,
    rootVisible: options.rootVisible,
    viewportOffset: options.viewportOffset,
  });
  const state: RootRuntimeState = {
    identified,
    treeRevision,
    geometryRevision,
    lease: options.state.lease,
  };
  return {
    state,
    messages: {
      commit,
      geometry,
      ready: state.lease.phase === "active",
    },
  };
}

function mapValuesToNodeKeys<T>(
  identified: IdentifiedRoot,
  values: Record<string, T>
): Record<string, T> {
  const mapped: Record<string, T> = { ...values };
  const walk = (nodes: IdentifiedRoot["children"]) => {
    for (const node of nodes) {
      const authorId = node.node.authorId;
      if (authorId && values[authorId] !== undefined) {
        mapped[node.nodeRef.nodeKey] = values[authorId];
      }
      walk(node.children);
    }
  };
  walk(identified.children);
  return mapped;
}

export function applyHostLeaseMessage(
  state: RootRuntimeState,
  message: { action?: string; leaseId?: string; sequence?: number; leaseDurationMs?: number },
  nowMs: number
): { state: RootRuntimeState; leaseAccept?: RootHostMessages["leaseAccept"]; ready: boolean } {
  if (message.action === "root.leaseGranted" && message.leaseId) {
    const granted = viewApplyGrant(
      message.leaseId,
      message.sequence ?? 1,
      message.leaseDurationMs ?? 8000,
      nowMs
    );
    const accepted = viewAcceptLease(granted);
    if (!accepted || !accepted.leaseId || !state.identified) {
      return { state: { ...state, lease: granted }, ready: false };
    }
    return {
      state: { ...state, lease: accepted },
      leaseAccept: {
        action: "root.leaseAccept",
        id: state.identified.rootRef.rootKey,
        root: state.identified.rootRef,
        leaseId: accepted.leaseId,
        sequence: accepted.sequence,
      },
      ready: false,
    };
  }
  if (message.action === "root.leaseActive") {
    const lease = viewMarkActive(state.lease);
    return { state: { ...state, lease }, ready: lease.phase === "active" };
  }
  return { state, ready: state.lease.phase === "active" };
}

export function rootFallbackAllowed(state: RootRuntimeState, nowMs: number): boolean {
  return viewCanShowFallback(state.lease, nowMs);
}

function mapRectsToNodeKeys(
  identified: IdentifiedRoot,
  rects: Record<string, { x: number; y: number; width: number; height: number }>
): Record<string, { x: number; y: number; width: number; height: number }> {
  const mapped: Record<string, { x: number; y: number; width: number; height: number }> = {
    ...rects,
  };
  const walk = (nodes: IdentifiedRoot["children"]) => {
    for (const node of nodes) {
      const authorId = node.node.authorId;
      if (authorId && rects[authorId]) {
        mapped[node.nodeRef.nodeKey] = rects[authorId];
      }
      walk(node.children);
    }
  };
  walk(identified.children);
  return mapped;
}
