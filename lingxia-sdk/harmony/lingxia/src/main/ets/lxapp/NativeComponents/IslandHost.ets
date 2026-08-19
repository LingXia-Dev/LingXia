/**
 * Inline native island host for HarmonyOS.
 *
 * Root container z-order follows document order. NodeContainer children
 * follow committed sibling order from `root.commit`, never message arrival.
 */
import {
  NativeComponentNodeController,
  NodeControllerParams
} from './NativeComponentNodeController';
import { NativeComponentRecord } from './NativeComponentTypes';

const ALLOWED_KINDS: string[] = ['root', 'view', 'text', 'tappable', 'slider', 'video'];

class IslandNode {
  key: string = '';
  kind: string = '';
  order: number = 0;
  controller: NativeComponentNodeController = new NativeComponentNodeController();
  zIndex: number = 0;
}

export class InlineNativeIslandHost {
  private nodes: Map<string, IslandNode> = new Map();
  private lastRevision: number = 0;
  private compositionOrder: string[] = [];
  private nodeContainers: NativeComponentNodeController[] = [];

  handle(action: string, payload: Record<string, Object>): boolean {
    if (action === 'root.commit') {
      this.applyCommit(payload);
      return true;
    }
    if (action === 'geometry.snapshot' || action === 'video.command' || action === 'root.leaseAccept') {
      return true;
    }
    return false;
  }

  lastAppliedRevision(): number {
    return this.lastRevision;
  }

  orderedKeys(): string[] {
    return this.compositionOrder.slice();
  }

  orderedNodeContainers(): NativeComponentNodeController[] {
    return this.nodeContainers.slice();
  }

  private applyCommit(payload: Record<string, Object>): void {
    const operations = payload['operations'] as Object[] | undefined;
    if (!operations) {
      return;
    }
    for (let i = 0; i < operations.length; i++) {
      const op = operations[i] as Record<string, Object>;
      const kind = op['op'] as string;
      if (kind === 'mount') {
        this.mount(op['node'] as Record<string, Object>);
      } else if (kind === 'unmount') {
        const node = op['node'] as Record<string, Object>;
        const ref = (node['ref'] as Record<string, Object>) ?? node;
        const key = (ref['nodeKey'] as string) ?? '';
        this.nodes.delete(key);
      } else if (kind === 'reorder') {
        const node = op['node'] as Record<string, Object>;
        const ref = (node['ref'] as Record<string, Object>) ?? node;
        const key = (ref['nodeKey'] as string) ?? '';
        const existing = this.nodes.get(key);
        if (existing) {
          existing.order = op['order'] as number;
        }
      }
    }
    this.restack();
    const revision = payload['revision'] as number;
    if (revision !== undefined) {
      this.lastRevision = revision;
    }
  }

  private mount(node: Record<string, Object>): void {
    const ref = node['ref'] as Record<string, Object>;
    if (!ref) {
      return;
    }
    const key = ref['nodeKey'] as string;
    const kind = node['kind'] as string;
    if (ALLOWED_KINDS.indexOf(kind) < 0) {
      return;
    }
    const item = new IslandNode();
    item.key = key;
    item.kind = kind;
    item.order = (node['order'] as number) ?? 0;
    const params = new NodeControllerParams();
    params.componentId = (node['authorId'] as string) ?? key;
    params.type = kind === 'video' ? 'video.native' : `island/${kind}`;
    params.embedId = key;
    const props = new NativeComponentRecord();
    const rawProps = node['props'] as Record<string, Object> | undefined;
    if (rawProps) {
      Object.keys(rawProps).forEach((propKey: string) => {
        props.set(propKey, rawProps[propKey]);
      });
    }
    params.props = props;
    item.controller.setRenderOption(params);
    this.nodes.set(key, item);
  }

  private restack(): void {
    const values: IslandNode[] = [];
    this.nodes.forEach((node: IslandNode) => {
      values.push(node);
    });
    values.sort((a: IslandNode, b: IslandNode) => {
      if (a.order !== b.order) {
        return a.order - b.order;
      }
      return a.key < b.key ? -1 : 1;
    });
    this.compositionOrder = [];
    this.nodeContainers = [];
    for (let i = 0; i < values.length; i++) {
      values[i].zIndex = i;
      this.compositionOrder.push(values[i].key);
      this.nodeContainers.push(values[i].controller);
    }
  }
}
