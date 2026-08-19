import type { IdentifiedNode, IdentifiedRoot } from "./identity.js";
import type { RootRef } from "./types.js";

export interface NativeRootCommitJson {
  action: "root.commit";
  root: RootRef;
  baseRevision: number;
  revision: number;
  operations: NativeRootOperationJson[];
}

export type NativeRootOperationJson =
  | { op: "mount"; node: NativeNodeJson }
  | { op: "update"; node: IdentifiedNode["nodeRef"]; patch: Record<string, unknown> }
  | { op: "reparent"; node: IdentifiedNode["nodeRef"]; parent: IdentifiedNode["nodeRef"] | null }
  | { op: "reorder"; node: IdentifiedNode["nodeRef"]; order: number }
  | { op: "unmount"; node: IdentifiedNode["nodeRef"] };

export interface NativeNodeJson {
  ref: IdentifiedNode["nodeRef"];
  kind: string;
  parent: IdentifiedNode["nodeRef"] | null;
  order: number;
  authorType: string;
  authorId?: string;
  automationId?: string;
  props: Record<string, unknown>;
}

export function buildRootCommit(
  next: IdentifiedRoot,
  previous: IdentifiedRoot | null,
  revision: number
): NativeRootCommitJson {
  const baseRevision = previous ? revision - 1 : 0;
  const operations: NativeRootOperationJson[] = [];
  const prevIndex = previous ? indexIdentified(previous) : new Map<string, IdentifiedNode>();
  const nextIndex = indexIdentified(next);

  for (const [key, prev] of prevIndex) {
    if (!nextIndex.has(key)) {
      operations.push({ op: "unmount", node: prev.nodeRef });
    }
  }

  walkMounts(next.children, prevIndex, operations);

  for (const [key, current] of nextIndex) {
    const prev = prevIndex.get(key);
    if (!prev) continue;
    if (parentKey(prev) !== parentKey(current)) {
      operations.push({
        op: "reparent",
        node: current.nodeRef,
        parent: current.parentRef,
      });
    }
    if (prev.order !== current.order) {
      operations.push({ op: "reorder", node: current.nodeRef, order: current.order });
    }
    const patch = propsPatch(prev.node.props, current.node.props);
    if (patch) {
      operations.push({ op: "update", node: current.nodeRef, patch });
    }
  }

  return {
    action: "root.commit",
    root: next.rootRef,
    baseRevision,
    revision,
    operations,
  };
}

function walkMounts(
  nodes: readonly IdentifiedNode[],
  prevIndex: Map<string, IdentifiedNode>,
  operations: NativeRootOperationJson[]
): void {
  for (const node of nodes) {
    if (!prevIndex.has(node.nodeRef.nodeKey)) {
      operations.push({
        op: "mount",
        node: {
          ref: node.nodeRef,
          kind: node.node.kind,
          parent: node.parentRef,
          order: node.order,
          authorType: node.node.authorType,
          authorId: node.node.authorId,
          automationId: node.node.automationId,
          props: node.node.props,
        },
      });
    }
    walkMounts(node.children, prevIndex, operations);
  }
}

function indexIdentified(root: IdentifiedRoot): Map<string, IdentifiedNode> {
  const map = new Map<string, IdentifiedNode>();
  const walk = (nodes: readonly IdentifiedNode[]) => {
    for (const node of nodes) {
      map.set(node.nodeRef.nodeKey, node);
      walk(node.children);
    }
  };
  walk(root.children);
  return map;
}

function parentKey(node: IdentifiedNode): string {
  return node.parentRef?.nodeKey ?? "";
}

function propsPatch(
  prev: Record<string, unknown>,
  next: Record<string, unknown>
): Record<string, unknown> | null {
  const patch: Record<string, unknown> = {};
  let changed = false;
  const keys = new Set([...Object.keys(prev), ...Object.keys(next)]);
  for (const key of keys) {
    if (JSON.stringify(prev[key]) === JSON.stringify(next[key])) continue;
    patch[key] = next[key] === undefined ? null : next[key];
    changed = true;
  }
  return changed ? patch : null;
}
