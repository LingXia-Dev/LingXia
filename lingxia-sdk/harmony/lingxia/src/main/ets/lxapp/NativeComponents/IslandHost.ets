/**
 * Inline native island host for HarmonyOS.
 *
 * Root container z-order follows document order. NodeContainer children
 * follow committed sibling order from `root.commit`, never message arrival.
 */
import {
  NativeComponentNodeController,
  NodeControllerParams,
  NodeControllerUpdateParams
} from './NativeComponentNodeController';
import { NativeComponentRecord, NativeComponentValue } from './NativeComponentTypes';
import { LxNativeVideoPlayer } from '../APIs/media/player/LxNativeVideoPlayer';

const ALLOWED_KINDS: string[] = ['root', 'view', 'text', 'tappable', 'slider', 'video'];

export class IslandNode {
  key: string = '';
  kind: string = '';
  authorId: string = '';
  order: number = 0;
  controller: NativeComponentNodeController = new NativeComponentNodeController();
  props: NativeComponentRecord = new NativeComponentRecord();
  zIndex: number = 0;
  rectX: number = 0;
  rectY: number = 0;
  rectW: number = 0;
  rectH: number = 0;
}

export class InlineNativeIslandHost {
  private nodes: Map<string, IslandNode> = new Map();
  private lastRevision: number = 0;
  private compositionOrder: string[] = [];
  private nodeContainers: NativeComponentNodeController[] = [];
  private pendingOutgoing: NativeComponentRecord[] = [];
  private leaseGranted: boolean = false;
  private leaseActive: boolean = false;
  private leaseId: string = '';
  private leaseSequence: number = 1;
  private lastRoot: Record<string, Object> | undefined = undefined;

  handle(action: string, payload: Record<string, Object>): boolean {
    if (action === 'root.commit') {
      this.applyCommit(payload);
      return true;
    }
    if (action === 'geometry.snapshot') {
      this.applyGeometry(payload);
      return true;
    }
    if (action === 'root.leaseAccept') {
      this.acceptLease(payload);
      return true;
    }
    if (action === 'video.command') {
      this.applyVideoCommand(payload);
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

  canDisplay(): boolean {
    return this.leaseActive;
  }

  drainOutgoing(): NativeComponentRecord[] {
    const outgoing = this.pendingOutgoing;
    this.pendingOutgoing = [];
    return outgoing;
  }

  /**
   * Release island controllers and reset the lease for a destroyed WebView.
   * A subsequent root commit must not inherit stale ArkUI nodes or state.
   */
  teardown(): void {
    this.nodes.forEach((node: IslandNode) => {
      if (node.kind === 'video' && node.authorId.length > 0) {
        LxNativeVideoPlayer.destroyStatic(node.authorId);
      }
      node.controller.setBuilderNode(null);
      node.controller.rebuild();
    });
    this.nodes.clear();
    this.compositionOrder = [];
    this.nodeContainers = [];
    this.pendingOutgoing = [];
    this.leaseGranted = false;
    this.leaseActive = false;
    this.leaseId = '';
    this.leaseSequence = 1;
    this.lastRoot = undefined;
    this.lastRevision = 0;
  }

  videoAuthorIds(): string[] {
    const ids: string[] = [];
    this.nodes.forEach((node: IslandNode) => {
      if (node.kind === 'video' && node.authorId.length > 0) {
        ids.push(node.authorId);
      }
    });
    return ids;
  }

  adoptEntries(): IslandNode[] {
    const values: IslandNode[] = [];
    this.nodes.forEach((node: IslandNode) => {
      values.push(node);
    });
    return values;
  }

  controllerForAuthor(authorId: string): NativeComponentNodeController | undefined {
    let found: NativeComponentNodeController | undefined = undefined;
    this.nodes.forEach((node: IslandNode) => {
      if (node.authorId === authorId) {
        found = node.controller;
      }
    });
    return found;
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
      } else if (kind === 'update') {
        this.updateNode(op);
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
    const root = payload['root'] as Record<string, Object> | undefined;
    if (root) {
      this.lastRoot = root;
      this.grantLeaseIfNeeded(root);
    }
  }

  private applyGeometry(payload: Record<string, Object>): void {
    const nodes = payload['nodes'] as Object[] | undefined;
    if (!nodes) {
      return;
    }
    for (let i = 0; i < nodes.length; i++) {
      const entry = nodes[i] as Record<string, Object>;
      const ref = entry['ref'] as Record<string, Object>;
      if (!ref) {
        continue;
      }
      const key = (ref['nodeKey'] as string) ?? '';
      const existing = this.nodes.get(key);
      const rect = entry['contentRect'] as Record<string, Object> | undefined;
      if (!existing || !rect) {
        continue;
      }
      existing.rectX = (rect['x'] as number) ?? 0;
      existing.rectY = (rect['y'] as number) ?? 0;
      existing.rectW = (rect['width'] as number) ?? 0;
      existing.rectH = (rect['height'] as number) ?? 0;
      const update = new NodeControllerUpdateParams();
      update.x = existing.rectX;
      update.y = existing.rectY;
      update.width = existing.rectW;
      update.height = existing.rectH;
      existing.controller.updateRenderOption(update);
    }
  }

  private acceptLease(payload: Record<string, Object>): void {
    if (!this.leaseGranted || this.leaseActive) {
      return;
    }
    const incomingId = (payload['leaseId'] as string) ?? '';
    const incomingSeq = (payload['sequence'] as number) ?? 0;
    if (incomingId.length > 0 && incomingId !== this.leaseId) {
      return;
    }
    if (incomingSeq > 0 && incomingSeq !== this.leaseSequence) {
      return;
    }
    this.leaseActive = true;
    const active = new NativeComponentRecord();
    active.set('action', 'root.leaseActive');
    const rootKey = this.lastRoot ? (this.lastRoot['rootKey'] as string) : 'island';
    active.set('id', rootKey);
    if (this.lastRoot) {
      active.set('root', this.lastRoot);
    }
    active.set('leaseId', this.leaseId);
    active.set('sequence', this.leaseSequence);
    this.pendingOutgoing.push(active);
  }

  private applyVideoCommand(payload: Record<string, Object>): void {
    const command = payload['command'] as Record<string, Object> | undefined;
    const owner = payload['owner'] as Record<string, Object> | undefined;
    const name = command ? (command['name'] as string) : '';
    let authorId = '';
    if (owner) {
      const key = (owner['nodeKey'] as string) ?? '';
      const node = this.nodes.get(key);
      if (node) {
        authorId = node.authorId.length > 0 ? node.authorId : key;
      }
    }
    if (authorId.length === 0) {
      const ids = this.videoAuthorIds();
      if (ids.length > 0) {
        authorId = ids[0];
      }
    }
    if (authorId.length === 0) {
      return;
    }
    if (name === 'play') {
      LxNativeVideoPlayer.playStatic(authorId);
    } else if (name === 'pause') {
      LxNativeVideoPlayer.pauseStatic(authorId);
    } else if (name === 'stop') {
      LxNativeVideoPlayer.stopStatic(authorId);
    } else if (name === 'seek') {
      const seconds = command ? (command['seconds'] as number) : 0;
      LxNativeVideoPlayer.seekTo(authorId, (seconds || 0) * 1000);
    } else if (name === 'enterFullscreen') {
      const controller = this.controllerForAuthor(authorId);
      if (controller) {
        controller.toggleFullscreen(0, 0);
      }
    }
  }

  private grantLeaseIfNeeded(root: Record<string, Object>): void {
    if (this.leaseGranted) {
      return;
    }
    const rootKey = (root['rootKey'] as string) ?? 'island';
    this.leaseGranted = true;
    this.leaseId = `lease-${rootKey}`;
    this.leaseSequence = 1;
    const grant = new NativeComponentRecord();
    grant.set('action', 'root.leaseGranted');
    grant.set('id', rootKey);
    grant.set('root', root);
    grant.set('leaseId', this.leaseId);
    grant.set('sequence', this.leaseSequence);
    grant.set('leaseDurationMs', 8000);
    this.pendingOutgoing.push(grant);
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
    item.authorId = (node['authorId'] as string) ?? '';
    item.order = (node['order'] as number) ?? 0;
    const params = new NodeControllerParams();
    params.componentId = item.authorId.length > 0 ? item.authorId : key;
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
    item.props = props;
    item.controller.setRenderOption(params);
    this.nodes.set(key, item);
  }

  private updateNode(op: Record<string, Object>): void {
    const node = op['node'] as Record<string, Object>;
    if (!node) {
      return;
    }
    const ref = (node['ref'] as Record<string, Object>) ?? node;
    const key = (ref['nodeKey'] as string) ?? '';
    const existing = this.nodes.get(key);
    if (!existing) {
      return;
    }
    const patch = op['patch'] as Record<string, Object> | undefined;
    if (!patch) {
      return;
    }
    const next = new NativeComponentRecord();
    existing.props.forEach((propKey: string, value: NativeComponentValue) => {
      next.set(propKey, value);
    });
    Object.keys(patch).forEach((propKey: string) => {
      const value = patch[propKey] as NativeComponentValue;
      if (value === null || value === undefined) {
        next.delete(propKey);
      } else {
        next.set(propKey, value);
      }
    });
    existing.props = next;
    const update = new NodeControllerUpdateParams();
    update.props = next;
    existing.controller.updateRenderOption(update);
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
