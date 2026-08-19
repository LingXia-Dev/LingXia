import type { CompiledNativeRoot, CoreNode, NodeRef, RootRef } from "./types.js";

export interface IdentityRecord {
  nodeKey: string;
  nodeEpoch: number;
  authorId?: string;
  path: string;
}

export interface IdentityTable {
  byPath: Record<string, IdentityRecord>;
  byAuthorId: Record<string, IdentityRecord>;
}

export interface IdentifiedNode {
  node: CoreNode;
  nodeRef: NodeRef;
  parentRef: NodeRef | null;
  order: number;
  path: string;
  children: IdentifiedNode[];
}

export interface IdentifiedRoot {
  rootRef: RootRef;
  children: IdentifiedNode[];
  table: IdentityTable;
}

let autoKey = 0;

export function nextOpaqueKey(prefix: string): string {
  autoKey += 1;
  return `${prefix}-${autoKey.toString(36)}`;
}

export function identifyCompiledRoot(
  compiled: CompiledNativeRoot,
  rootRef: RootRef,
  previous: IdentifiedRoot | null = null
): IdentifiedRoot {
  const table: IdentityTable = { byPath: {}, byAuthorId: {} };
  const livePrev = previous ? liveNodeKeys(previous) : new Set<string>();
  const children = compiled.children.map((child, index) =>
    identifyNode(child, rootRef, null, index, String(index), previous, livePrev, table)
  );
  return { rootRef, children, table };
}

function liveNodeKeys(root: IdentifiedRoot): Set<string> {
  const keys = new Set<string>();
  const walk = (nodes: readonly IdentifiedNode[]) => {
    for (const node of nodes) {
      keys.add(node.nodeRef.nodeKey);
      walk(node.children);
    }
  };
  walk(root.children);
  return keys;
}

function identifyNode(
  node: CoreNode,
  rootRef: RootRef,
  parentRef: NodeRef | null,
  order: number,
  path: string,
  previous: IdentifiedRoot | null,
  livePrev: Set<string>,
  table: IdentityTable
): IdentifiedNode {
  const authorId = node.authorId;
  const prev =
    (authorId ? previous?.table.byAuthorId[authorId] : undefined) ?? previous?.table.byPath[path];
  let nodeKey: string;
  let nodeEpoch: number;
  if (prev && livePrev.has(prev.nodeKey)) {
    nodeKey = prev.nodeKey;
    nodeEpoch = prev.nodeEpoch;
  } else if (prev && !livePrev.has(prev.nodeKey)) {
    nodeKey = prev.nodeKey;
    nodeEpoch = prev.nodeEpoch + 1;
  } else {
    nodeKey = nextOpaqueKey("n");
    nodeEpoch = 1;
  }
  const record: IdentityRecord = { nodeKey, nodeEpoch, authorId, path };
  table.byPath[path] = record;
  if (authorId) {
    table.byAuthorId[authorId] = record;
  }
  const nodeRef: NodeRef = {
    ...rootRef,
    nodeKey,
    nodeEpoch,
  };
  const children = node.children.map((child, index) =>
    identifyNode(child, rootRef, nodeRef, index, `${path}/${index}`, previous, livePrev, table)
  );
  return { node, nodeRef, parentRef, order, path, children };
}

export function remountIdentity(previous: IdentityRecord): IdentityRecord {
  return { ...previous, nodeEpoch: previous.nodeEpoch + 1 };
}
