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
import { validateInlineNativeMediaUrls } from 'liblingxia.so';

const ALLOWED_KINDS: string[] = ['root', 'view', 'text', 'tappable', 'slider', 'video'];

export class IslandNode {
  key: string = '';
  rootKey: string = '';
  kind: string = '';
  authorId: string = '';
  parentKey: string = '';
  order: number = 0;
  controller: NativeComponentNodeController = new NativeComponentNodeController();
  props: NativeComponentRecord = new NativeComponentRecord();
  zIndex: number = 0;
  rectX: number = 0;
  rectY: number = 0;
  rectW: number = 0;
  rectH: number = 0;
  visible: boolean = true;
}

class IslandLease {
  root: Record<string, Object> = {};
  id: string = '';
  sequence: number = 1;
  active: boolean = false;
}

export class InlineNativeIslandHost {
  private appId: string;
  private nodes: Map<string, IslandNode> = new Map();
  private lastRevision: number = 0;
  private revisions: Map<string, number> = new Map();
  private roots: Map<string, Record<string, Object>> = new Map();
  private rootOrders: Map<string, number> = new Map();
  private compositionOrder: string[] = [];
  private nodeContainers: NativeComponentNodeController[] = [];
  private pendingOutgoing: NativeComponentRecord[] = [];
  private leases: Map<string, IslandLease> = new Map();

  constructor(appId: string = 'app') {
    this.appId = appId;
  }

  handle(action: string, payload: Record<string, Object>): boolean {
    if (action === 'root.commit') {
      this.applyCommit(payload);
      return true;
    }
    if (action === 'root.destroy') {
      this.destroyRoot(payload);
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
    let active = false;
    this.leases.forEach((lease: IslandLease) => { active = active || lease.active; });
    return active;
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
      this.releaseNode(node);
    });
    this.nodes.clear();
    this.compositionOrder = [];
    this.nodeContainers = [];
    this.pendingOutgoing = [];
    this.leases.clear();
    this.revisions.clear();
    this.roots.clear();
    this.rootOrders.clear();
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
      if (this.leases.get(node.rootKey)?.active) values.push(node);
    });
    values.sort((a: IslandNode, b: IslandNode) => {
      if (a.order !== b.order) {
        return a.order - b.order;
      }
      return a.key < b.key ? -1 : 1;
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
    const root = payload['root'] as Record<string, Object> | undefined;
    if (!root) {
      return;
    }
    const rootKey = (root['rootKey'] as string) ?? '';
    const baseRevision = (payload['baseRevision'] as number) ?? 0;
    const revision = (payload['revision'] as number) ?? 0;
    if (revision <= baseRevision || revision === 0) {
      const error = new NativeComponentRecord();
      error.set('action', 'root.error');
      error.set('id', rootKey);
      error.set('root', root);
      error.set('message', 'commit revision must be greater than baseRevision');
      error.set('recoverable', true);
      this.pendingOutgoing.push(error);
      return;
    }
    const policyError: string = validateInlineNativeMediaUrls(
      this.appId,
      JSON.stringify(this.mediaUrls(payload))
    ) as string;
    if (policyError.length > 0) {
      const error = new NativeComponentRecord();
      error.set('action', 'root.error');
      error.set('id', rootKey);
      error.set('root', root);
      error.set('message', policyError);
      error.set('recoverable', true);
      this.pendingOutgoing.push(error);
      return;
    }
    const last = this.revisions.get(rootKey) ?? 0;
    const existingRoot = this.roots.get(rootKey);
    if (existingRoot && !this.sameRootGeneration(existingRoot, root)) {
      const error = new NativeComponentRecord();
      error.set('action', 'root.error');
      error.set('id', rootKey);
      error.set('root', root);
      error.set('message', 'root generation changed without destroy');
      error.set('recoverable', true);
      this.pendingOutgoing.push(error);
      return;
    }
    if (baseRevision === 0) {
      const stale: string[] = [];
      this.nodes.forEach((node: IslandNode, key: string) => {
        if (node.rootKey === rootKey) stale.push(key);
      });
      stale.forEach((key: string) => {
        const node = this.nodes.get(key);
        if (node) this.releaseNode(node);
        this.nodes.delete(key);
      });
      this.revisions.delete(rootKey);
      this.leases.delete(rootKey);
    } else if (baseRevision !== last) {
      const resync = new NativeComponentRecord();
      resync.set('action', 'root.resyncRequired');
      resync.set('id', rootKey);
      resync.set('root', root);
      resync.set('lastAppliedRevision', last);
      this.pendingOutgoing.push(resync);
      return;
    }
    const operations = payload['operations'] as Object[] | undefined;
    if (!operations) {
      return;
    }
    for (let i = 0; i < operations.length; i++) {
      const op = operations[i] as Record<string, Object>;
      const kind = op['op'] as string;
      if (kind === 'mount') {
        this.mount(op['node'] as Record<string, Object>, rootKey);
      } else if (kind === 'update') {
        this.updateNode(op);
      } else if (kind === 'unmount') {
        const node = op['node'] as Record<string, Object>;
        const ref = (node['ref'] as Record<string, Object>) ?? node;
        if (((ref['rootKey'] as string) ?? '') !== rootKey) continue;
        const key = (ref['nodeKey'] as string) ?? '';
        const existing = this.nodes.get(key);
        if (existing) {
          this.releaseNode(existing);
          this.nodes.delete(key);
        }
      } else if (kind === 'reorder') {
        const node = op['node'] as Record<string, Object>;
        const ref = (node['ref'] as Record<string, Object>) ?? node;
        if (((ref['rootKey'] as string) ?? '') !== rootKey) continue;
        const key = (ref['nodeKey'] as string) ?? '';
        const existing = this.nodes.get(key);
        if (existing) {
          existing.order = op['order'] as number;
        }
      } else if (kind === 'reparent') {
        const node = op['node'] as Record<string, Object>;
        const ref = (node['ref'] as Record<string, Object>) ?? node;
        if (((ref['rootKey'] as string) ?? '') !== rootKey) continue;
        const key = (ref['nodeKey'] as string) ?? '';
        const existing = this.nodes.get(key);
        const parent = op['parent'] as Record<string, Object> | undefined;
        if (parent && ((parent['rootKey'] as string) ?? '') !== rootKey) continue;
        if (existing) existing.parentKey = parent ? ((parent['nodeKey'] as string) ?? '') : '';
      }
    }
    this.restack();
    this.revisions.set(rootKey, revision);
    this.roots.set(rootKey, root);
    this.lastRevision = Math.max(...Array.from(this.revisions.values()));
    const applied = new NativeComponentRecord();
    applied.set('action', 'root.applied');
    applied.set('id', rootKey);
    applied.set('root', root);
    applied.set('revision', revision);
    this.pendingOutgoing.push(applied);
    this.grantLeaseIfNeeded(root);
  }

  private destroyRoot(payload: Record<string, Object>): void {
    const root = payload['root'] as Record<string, Object> | undefined;
    const rootKey = root ? ((root['rootKey'] as string) ?? '') : '';
    const current = this.roots.get(rootKey);
    if (!root || !current || !this.sameRootGeneration(current, root)) return;
    const stale: string[] = [];
    this.nodes.forEach((node: IslandNode, key: string) => {
      if (node.rootKey === rootKey) stale.push(key);
    });
    stale.forEach((key: string) => {
      const node = this.nodes.get(key);
      if (node) this.releaseNode(node);
      this.nodes.delete(key);
    });
    this.revisions.delete(rootKey);
    this.roots.delete(rootKey);
    this.rootOrders.delete(rootKey);
    this.leases.delete(rootKey);
    this.lastRevision = this.revisions.size > 0 ? Math.max(...Array.from(this.revisions.values())) : 0;
    this.restack();
  }

  private applyGeometry(payload: Record<string, Object>): void {
    const roots = payload['roots'] as Object[] | undefined;
    if (roots) {
      roots.forEach((raw: Object) => {
        const entry = raw as Record<string, Object>;
        const ref = entry['ref'] as Record<string, Object> | undefined;
        const rootKey = ref ? ((ref['rootKey'] as string) ?? '') : '';
        const current = this.roots.get(rootKey);
        if (ref && current && this.sameRootGeneration(current, ref)) {
          this.rootOrders.set(rootKey, (entry['rootOrder'] as number) ?? Number.MAX_SAFE_INTEGER);
        }
      });
    }
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
      if (!existing || !rect || ((ref['rootKey'] as string) ?? '') !== existing.rootKey) {
        continue;
      }
      existing.rectX = (rect['x'] as number) ?? 0;
      existing.rectY = (rect['y'] as number) ?? 0;
      existing.rectW = (rect['width'] as number) ?? 0;
      existing.rectH = (rect['height'] as number) ?? 0;
      existing.visible = (entry['visible'] as boolean) ?? true;
      const update = new NodeControllerUpdateParams();
      update.x = existing.rectX;
      update.y = existing.rectY;
      update.width = existing.visible ? Math.max(existing.rectW, 1) : 0;
      update.height = existing.visible ? Math.max(existing.rectH, 1) : 0;
      existing.controller.updateRenderOption(update);
    }
    this.restack();
  }

  private acceptLease(payload: Record<string, Object>): void {
    const root = payload['root'] as Record<string, Object> | undefined;
    const rootKey = (payload['id'] as string) ?? (root ? (root['rootKey'] as string) : '');
    const lease = this.leases.get(rootKey);
    if (!lease || lease.active) {
      return;
    }
    const incomingId = (payload['leaseId'] as string) ?? '';
    const incomingSeq = (payload['sequence'] as number) ?? 0;
    if (incomingId.length > 0 && incomingId !== lease.id) {
      return;
    }
    if (incomingSeq > 0 && incomingSeq !== lease.sequence) {
      return;
    }
    lease.active = true;
    const active = new NativeComponentRecord();
    active.set('action', 'root.leaseActive');
    active.set('id', rootKey);
    active.set('root', lease.root);
    active.set('leaseId', lease.id);
    active.set('sequence', lease.sequence);
    this.pendingOutgoing.push(active);
  }

  private applyVideoCommand(payload: Record<string, Object>): void {
    const policyError: string = validateInlineNativeMediaUrls(
      this.appId,
      JSON.stringify(this.mediaUrls(payload))
    ) as string;
    if (policyError.length > 0) {
      return;
    }
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
    } else if (name === 'exitFullscreen') {
      LxNativeVideoPlayer.exitFullscreen(authorId);
    }
  }

  private grantLeaseIfNeeded(root: Record<string, Object>): void {
    const rootKey = (root['rootKey'] as string) ?? 'island';
    if (this.leases.has(rootKey)) return;
    const lease = new IslandLease();
    lease.root = root;
    lease.id = `lease-${rootKey}`;
    this.leases.set(rootKey, lease);
    const grant = new NativeComponentRecord();
    grant.set('action', 'root.leaseGranted');
    grant.set('id', rootKey);
    grant.set('root', root);
    grant.set('leaseId', lease.id);
    grant.set('sequence', lease.sequence);
    grant.set('leaseDurationMs', 8000);
    this.pendingOutgoing.push(grant);
  }

  private mediaUrls(value: Object): string[] {
    const urls: string[] = [];
    const walk = (current: Object, key: string = ''): void => {
      if (typeof current === 'string') {
        if (['src', 'url', 'uri', 'poster'].indexOf(key) >= 0) urls.push(current);
        return;
      }
      if (Array.isArray(current)) {
        current.forEach((item: Object) => walk(item, key));
        return;
      }
      if (current && typeof current === 'object') {
        Object.keys(current).forEach((childKey: string) => {
          walk((current as Record<string, Object>)[childKey], childKey);
        });
      }
    };
    walk(value);
    return urls;
  }

  private mount(node: Record<string, Object>, expectedRootKey: string): void {
    const ref = node['ref'] as Record<string, Object>;
    if (!ref) {
      return;
    }
    const key = ref['nodeKey'] as string;
    const rootKey = (ref['rootKey'] as string) ?? '';
    if (rootKey !== expectedRootKey) return;
    const kind = node['kind'] as string;
    if (ALLOWED_KINDS.indexOf(kind) < 0) {
      return;
    }
    const previous = this.nodes.get(key);
    if (previous && previous.rootKey !== rootKey) return;
    if (previous) {
      this.releaseNode(previous);
    }
    const item = new IslandNode();
    item.key = key;
    item.rootKey = (ref['rootKey'] as string) ?? '';
    item.kind = kind;
    item.authorId = (node['authorId'] as string) ?? '';
    const parent = node['parent'] as Record<string, Object> | undefined;
    if (parent && ((parent['rootKey'] as string) ?? '') !== rootKey) return;
    item.parentKey = parent ? ((parent['nodeKey'] as string) ?? '') : '';
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
    const automationId = node['automationId'] as string | undefined;
    if (automationId && automationId.length > 0) {
      props.set('automationId', automationId);
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
    if (!existing || ((ref['rootKey'] as string) ?? '') !== existing.rootKey) {
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
    const append = (parentKey: string): void => {
      const children: IslandNode[] = [];
      this.nodes.forEach((node: IslandNode) => {
        if (node.parentKey === parentKey) children.push(node);
      });
      children.sort((a: IslandNode, b: IslandNode) => {
        if (parentKey.length === 0) {
          const rootOrder = (this.rootOrders.get(a.rootKey) ?? Number.MAX_SAFE_INTEGER) -
            (this.rootOrders.get(b.rootKey) ?? Number.MAX_SAFE_INTEGER);
          if (rootOrder !== 0) return rootOrder;
        }
        if (a.order !== b.order) return a.order - b.order;
        return a.key < b.key ? -1 : 1;
      });
      children.forEach((node: IslandNode) => {
        values.push(node);
        append(node.key);
      });
    };
    append('');
    this.compositionOrder = [];
    this.nodeContainers = [];
    for (let i = 0; i < values.length; i++) {
      values[i].zIndex = i;
      this.compositionOrder.push(values[i].key);
      this.nodeContainers.push(values[i].controller);
    }
  }

  private sameRootGeneration(
    lhs: Record<string, Object>,
    rhs: Record<string, Object>
  ): boolean {
    return lhs['surfaceInstanceId'] === rhs['surfaceInstanceId'] &&
      lhs['pageInstanceId'] === rhs['pageInstanceId'] &&
      lhs['documentInstanceId'] === rhs['documentInstanceId'] &&
      lhs['rootKey'] === rhs['rootKey'] &&
      lhs['rootEpoch'] === rhs['rootEpoch'];
  }

  private releaseNode(node: IslandNode): void {
    if (node.kind === 'video') {
      const componentId = node.authorId.length > 0 ? node.authorId : node.key;
      LxNativeVideoPlayer.destroyStatic(componentId);
    }
    node.controller.setBuilderNode(null);
    node.controller.rebuild();
  }
}
