/**
 * Inline native island host for HarmonyOS.
 *
 * Root container z-order follows document order. NodeContainer children
 * follow committed sibling order from `root.commit`, never message arrival.
 */

const ALLOWED_KINDS: string[] = ['root', 'view', 'text', 'tappable', 'slider', 'video'];

class IslandNode {
  key: string = '';
  kind: string = '';
  order: number = 0;
}

export class InlineNativeIslandHost {
  private nodes: Map<string, IslandNode> = new Map();
  private lastRevision: number = 0;
  private compositionOrder: string[] = [];

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
        const key = node['nodeKey'] as string;
        this.nodes.delete(key);
      } else if (kind === 'reorder') {
        const node = op['node'] as Record<string, Object>;
        const key = node['nodeKey'] as string;
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
    this.compositionOrder = values.map((node: IslandNode) => node.key);
  }
}
