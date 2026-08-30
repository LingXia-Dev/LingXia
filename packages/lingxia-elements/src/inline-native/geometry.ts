import type { IdentifiedRoot } from "./identity.js";
import type { RootRef } from "./types.js";

export interface NativeGeometrySnapshotJson {
  action: "geometry.snapshot";
  surfaceInstanceId: string;
  pageInstanceId: string;
  documentInstanceId: string;
  revision: number;
  coordinateSpace: "page-unscrolled-css-px";
  roots: Array<{
    ref: RootRef;
    basisTreeRevision: number;
    rootOrder: number;
    chainKey: string;
    contentRect: { x: number; y: number; width: number; height: number };
    visible: boolean;
  }>;
  nodes: Array<{
    ref: IdentifiedRoot["children"][number]["nodeRef"];
    chainKey: string;
    contentRect: { x: number; y: number; width: number; height: number };
    clipStack: unknown[];
    visible: boolean;
  }>;
  chains: Array<{
    chainKey: string;
    ancestors: Array<{
      key: string;
      viewportRect: { x: number; y: number; width: number; height: number };
      offsetX: number;
      offsetY: number;
    }>;
  }>;
}

export function buildGeometrySnapshot(options: {
  identified: IdentifiedRoot;
  basisTreeRevision: number;
  geometryRevision: number;
  rootOrder: number;
  rootRect: { x: number; y: number; width: number; height: number };
  nodeRects: Record<string, { x: number; y: number; width: number; height: number }>;
  nodeVisibility?: Record<string, boolean>;
  nodeClipStacks?: Record<string, unknown[]>;
  rootVisible?: boolean;
}): NativeGeometrySnapshotJson {
  const root = options.identified.rootRef;
  const nodes: NativeGeometrySnapshotJson["nodes"] = [];
  const walk = (list: IdentifiedRoot["children"]) => {
    for (const node of list) {
      nodes.push({
        ref: node.nodeRef,
        chainKey: "page",
        contentRect: options.nodeRects[node.nodeRef.nodeKey] ?? options.rootRect,
        clipStack: options.nodeClipStacks?.[node.nodeRef.nodeKey] ?? [],
        visible: options.nodeVisibility?.[node.nodeRef.nodeKey] ?? true,
      });
      walk(node.children);
    }
  };
  walk(options.identified.children);
  return {
    action: "geometry.snapshot",
    surfaceInstanceId: root.surfaceInstanceId,
    pageInstanceId: root.pageInstanceId,
    documentInstanceId: root.documentInstanceId,
    revision: options.geometryRevision,
    coordinateSpace: "page-unscrolled-css-px",
    roots: [
      {
        ref: root,
        basisTreeRevision: options.basisTreeRevision,
        rootOrder: options.rootOrder,
        chainKey: "page",
        contentRect: options.rootRect,
        visible: options.rootVisible ?? true,
      },
    ],
    nodes,
    chains: [{ chainKey: "page", ancestors: [] }],
  };
}
