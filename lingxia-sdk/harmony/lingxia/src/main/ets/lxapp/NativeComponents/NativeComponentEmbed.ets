/**
 * NativeComponentEmbed - Embed native components in WebView using official ArkWeb pattern
 * This module provides the embed handlers for Web component
 */
import { NodeRenderType } from '@kit.ArkUI';
import {
  NativeComponentEmptyDetail,
  NativeComponentRecord,
  NativeComponentValue,
  nativeComponentError,
  nativeComponentLog
} from './NativeComponentTypes';
import { getComponentCallback, removeComponentCallback } from './NativeCallbackRegistry';
import { onNativeComponentEvent, onCallback } from 'liblingxia.so';
import { NativeComponentNodeController, NodeControllerParams, NodeControllerUpdateParams } from './NativeComponentNodeController';
import { LxNativeVideoPlayer } from '../media/LxNativeVideoPlayer';

const PROPS_DATA_PREFIX: string = 'data:application/json,';

/**
 * Command object for component lifecycle control
 */
class ComponentCommand {
  __command: string = '';

  constructor(command: string) {
    this.__command = command;
  }
}

class WebTagRoute {
  appId: string = '';
  path: string = '';
}

interface EmbedInfoLike {
  get?: (key: string) => NativeComponentValue;
  src?: string;
  x?: NativeComponentValue;
  y?: NativeComponentValue;
  width?: NativeComponentValue;
  height?: NativeComponentValue;
}

interface EmbedDataLike {
  info?: EmbedInfoLike;
  src?: string;
  x?: NativeComponentValue;
  y?: NativeComponentValue;
  width?: NativeComponentValue;
  height?: NativeComponentValue;
}

/**
 * Manager for handling same-level embedding in a Web component
 */
export class NativeComponentEmbedManager {
  private webTag: string;
  private nodeControllerMap: Map<string, NativeComponentNodeController> = new Map();
  private componentIdArr: string[] = [];
  private pendingPropsByComponentId: Map<string, NativeComponentRecord> = new Map();
  private uiContext: UIContext | null = null;
  private onComponentsChanged?: () => void;
  private eventDispatcher?: (payload: NativeComponentRecord) => void;
  private pendingUnmountTimers: Map<string, number> = new Map();
  private componentPageFuncBindings: Map<string, Map<string, string>> = new Map();
  private componentDataset: Map<string, NativeComponentRecord> = new Map();
  private componentPlaybackIntent: Map<string, boolean> = new Map();
  private componentsPendingAutoResume: Set<string> = new Set();
  private lifecyclePaused: boolean = false;
  private componentAdjustPosition: Map<string, boolean> = new Map();
  private focusedComponentId: string = '';
  private focusedComponentType: string = '';
  private focusedAdjustPosition: boolean = true;

  constructor(webTag: string, eventDispatcher?: (payload: NativeComponentRecord) => void) {
    this.webTag = webTag;
    this.eventDispatcher = eventDispatcher;
  }

  /**
   * Set UI context for px2vp conversion
   */
  setUIContext(uiContext: UIContext): void {
    this.uiContext = uiContext;
  }

  /**
   * Set callback for component list changes
   */
  setOnComponentsChanged(callback: () => void): void {
    this.onComponentsChanged = callback;
  }

  setEventDispatcher(dispatcher?: (payload: NativeComponentRecord) => void): void {
    this.eventDispatcher = dispatcher;
  }

  /**
   * Get current component ID array (for ForEach rendering)
   */
  getComponentIds(): string[] {
    return this.componentIdArr;
  }

  getFocusedComponentType(): string {
    return this.focusedComponentType;
  }

  getFocusedAdjustPosition(): boolean {
    return this.focusedAdjustPosition;
  }

  /**
   * Get NodeController for a component ID
   */
  getNodeController(componentId: string): NativeComponentNodeController | undefined {
    return this.nodeControllerMap.get(componentId);
  }

  /**
   * Handle native embed lifecycle change event from Web component
   * This should be called from Web.onNativeEmbedLifecycleChange
   */
  handleLifecycleChange(embed: NativeEmbedDataInfo): void {
    const componentId = embed.info?.id?.toString() || '';
    if (!componentId) {
      nativeComponentError('Embed lifecycle event missing component ID');
      return;
    }

    if (embed.status === NativeEmbedStatus.CREATE) {
      this.handleCreate(componentId, embed);
    } else if (embed.status === NativeEmbedStatus.UPDATE) {
      this.handleUpdate(componentId, embed);
    } else if (embed.status === NativeEmbedStatus.DESTROY) {
      this.handleDestroy(componentId);
    }
  }

  private handleCreate(componentId: string, embed: NativeEmbedDataInfo): void {
    this.cancelPendingUnmount(componentId);
    // Check if component already exists - avoid duplicate creation
    if (this.nodeControllerMap.has(componentId)) {
      this.handleUpdate(componentId, embed);
      return;
    }

    const nodeController = new NativeComponentNodeController();

    // Parse component type from embed type (e.g., 'native/video' -> 'video.native')
    const embedType = embed.info?.type || 'native/component';
    const componentType = this.parseComponentType(embedType);

    // Get props from embed tag attribute
    const props = this.parseEmbedProps(embed);
    const pendingProps = this.pendingPropsByComponentId.get(componentId);
    if (pendingProps) {
      pendingProps.forEach((key: string, value: NativeComponentValue) => {
        props.set(key, value);
      });
      this.pendingPropsByComponentId.delete(componentId);
    }
    if (componentType === 'video.native') {
      this.applyVideoRotationFromProps(componentId, props);
      nativeComponentLog(
        `embed create(video props): componentId=${componentId} contentRotate=${String(props.get('contentRotate'))} clearContentRotate=${this.shouldClearProp(props, 'contentRotate')}`
      );
    }
    this.updateComponentAdjustPosition(componentId, props);
    this.updatePageFuncBindings(componentId, props);
    this.updateComponentDataset(componentId, props);

    const xPx = this.readEmbedInfoNumber(embed, 'x');
    const yPx = this.readEmbedInfoNumber(embed, 'y');
    const widthPx = this.readEmbedInfoNumber(embed, 'width');
    const heightPx = this.readEmbedInfoNumber(embed, 'height');
    const xVp = this.uiContext?.px2vp(xPx) || 0;
    const yVp = this.uiContext?.px2vp(yPx) || 0;
    const widthVp = this.uiContext?.px2vp(widthPx) || 0;
    const heightVp = this.uiContext?.px2vp(heightPx) || 0;

    const params = new NodeControllerParams();
    params.surfaceId = embed.surfaceId || '';
    params.type = componentType;
    // Use TEXTURE render type to preserve ArkWeb same-level semantics and coordinate mapping.
    // (DISPLAY may break overlay/layout for same-level components.)
    params.renderType = NodeRenderType.RENDER_TYPE_TEXTURE;
    params.embedId = embed.embedId || '';
    params.x = xVp;
    params.y = yVp;
    params.width = widthVp;
    params.height = heightVp;
    params.componentId = componentId;
    params.props = props;

    nodeController.setRenderOption(params);
    if (componentType === 'video.native') {
      nativeComponentLog(
        `embed create(video): componentId=${componentId} embedId=${params.embedId} surfaceId=${params.surfaceId} renderType=${params.renderType} size=${widthVp}x${heightVp}`
      );
    }

    // Set fullscreen callback to trigger UI update
    nodeController.setFullscreenCallback(() => {
      this.onComponentsChanged?.();
    });

    nodeController.rebuild();

    this.nodeControllerMap.set(componentId, nodeController);
    this.componentIdArr.push(componentId);

    this.onComponentsChanged?.();
  }

  private handleUpdate(componentId: string, embed: NativeEmbedDataInfo): void {
    this.cancelPendingUnmount(componentId);
    const nodeController = this.nodeControllerMap.get(componentId);
    if (!nodeController) return;

    const isVideoComponent = nodeController.getComponentType() === 'video.native';

    // Update layout sizes (vp) when embed dimensions change
    const xPx = this.readEmbedInfoNumber(embed, 'x');
    const yPx = this.readEmbedInfoNumber(embed, 'y');
    const widthPx = this.readEmbedInfoNumber(embed, 'width');
    const heightPx = this.readEmbedInfoNumber(embed, 'height');
    const xVp = this.uiContext?.px2vp(xPx) || 0;
    const yVp = this.uiContext?.px2vp(yPx) || 0;
    const widthVp = this.uiContext?.px2vp(widthPx) || 0;
    const heightVp = this.uiContext?.px2vp(heightPx) || 0;

    const updateParams = new NodeControllerUpdateParams();
    updateParams.x = xVp;
    updateParams.y = yVp;
    updateParams.width = widthVp;
    updateParams.height = heightVp;
    // Keep UPDATE props path enabled for all component types (including video).
    // This provides a robust fallback when bridge-driven updates are delayed.
    const props = this.parseEmbedProps(embed);
    this.updateComponentAdjustPosition(componentId, props);
    this.updatePageFuncBindings(componentId, props);
    this.updateComponentDataset(componentId, props);
    updateParams.props = props;
    nodeController.updateRenderOption(updateParams);
    if (isVideoComponent) {
      this.applyVideoRotationFromProps(componentId, props);
      nativeComponentLog(`embed update(video): componentId=${componentId} size=${widthVp}x${heightVp}`);
    }

    // Notify listeners so NodeContainer layout refreshes
    this.onComponentsChanged?.();
  }

  private handleDestroy(componentId: string): void {
    const nodeController = this.nodeControllerMap.get(componentId);
    if (!nodeController) return;

    // NativeEmbedStatus.DESTROY can happen during transient rebuilds (e.g. props/layout updates)
    // where the same componentId is recreated immediately after. Debounce teardown so we don't
    // stop a live stream mid-startup and end up in "loading forever".
    this.scheduleUnmount(componentId);
    if (nodeController.getComponentType() === 'video.native') {
      nativeComponentLog(`embed destroy(video): componentId=${componentId}`);
    }

    nodeController.setBuilderNode(null);
    nodeController.rebuild();

    this.nodeControllerMap.delete(componentId);
    this.componentAdjustPosition.delete(componentId);
    if (this.focusedComponentId === componentId) {
      this.focusedComponentId = '';
      this.focusedComponentType = '';
      this.focusedAdjustPosition = true;
    }
    this.componentPageFuncBindings.delete(componentId);
    this.componentDataset.delete(componentId);
    this.componentPlaybackIntent.delete(componentId);
    this.componentsPendingAutoResume.delete(componentId);
    const index = this.componentIdArr.indexOf(componentId);
    if (index > -1) {
      this.componentIdArr.splice(index, 1);
    }

    this.onComponentsChanged?.();
  }

  private cancelPendingUnmount(componentId: string): void {
    const timerId = this.pendingUnmountTimers.get(componentId);
    if (timerId !== undefined) {
      clearTimeout(timerId);
      this.pendingUnmountTimers.delete(componentId);
    }
  }

  private scheduleUnmount(componentId: string): void {
    this.cancelPendingUnmount(componentId);
    const delayMs = LxNativeVideoPlayer.isKeepAlive(componentId) ? 4000 : 1500;
    const timerId = setTimeout(() => {
      this.pendingUnmountTimers.delete(componentId);
      // If the component was recreated quickly, treat this destroy as a surface swap, not an unmount.
      if (this.nodeControllerMap.has(componentId)) return;

      // During fullscreen enter/exit, ArkWeb can DESTROY the embed and recreate it with the same
      // componentId after a longer layout pass. Don't tear down the stream in that window.
      if (LxNativeVideoPlayer.isKeepAlive(componentId)) {
        this.scheduleUnmount(componentId);
        return;
      }

      const callbackId = getComponentCallback(componentId);
      if (!callbackId) return;

      const payload = new NativeComponentRecord();
      payload.set('action', 'component.event');
      payload.set('id', componentId);
      payload.set('componentId', componentId);
      payload.set('event', 'unmount');
      payload.set('detail', new NativeComponentEmptyDetail());
      onCallback(callbackId, true, payload.toJSONString());
      removeComponentCallback(componentId);
    }, delayMs);
    this.pendingUnmountTimers.set(componentId, timerId);
  }

  /**
   * Handle gesture event from Web component
   * This should be called from Web.onNativeEmbedGestureEvent
   */
  handleGestureEvent(touch: NativeEmbedTouchInfo): void {
    let matchedCount = 0;
    let targetComponentId = '';
    this.componentIdArr.forEach((componentId) => {
      const nodeController = this.nodeControllerMap.get(componentId);
      if (nodeController && nodeController.getEmbedId() === touch.embedId) {
        matchedCount += 1;
        targetComponentId = componentId;
        nodeController.postEvent(touch.touchEvent);
      }
    });
    if (matchedCount !== 1) {
      nativeComponentLog(
        `gesture dispatch mismatch embedId=${String(touch.embedId)} matched=${matchedCount} target=${targetComponentId}`
      );
    }
  }

  /**
   * Push fresh component props from Web -> native (primary Harmony path).
   */
  updateComponentProps(componentId: string, propsJson: string): boolean {
    const freshProps = this.decodeProps(propsJson);
    if (!freshProps) {
      nativeComponentLog(
        `updateComponentProps: decode failed componentId=${componentId} prefix=${propsJson.substring(0, 64)}`
      );
      return false;
    }

    const nodeController = this.nodeControllerMap.get(componentId);
    if (!nodeController) {
      this.pendingPropsByComponentId.set(componentId, freshProps);
      this.updateComponentAdjustPosition(componentId, freshProps);
      this.updatePageFuncBindings(componentId, freshProps);
      this.updateComponentDataset(componentId, freshProps);
      // Treat as accepted. Pending props are applied once CREATE arrives.
      return true;
    }

    this.pendingPropsByComponentId.delete(componentId);
    this.updateComponentAdjustPosition(componentId, freshProps);
    this.updatePageFuncBindings(componentId, freshProps);
    this.updateComponentDataset(componentId, freshProps);

    const updateParams = new NodeControllerUpdateParams();
    updateParams.props = freshProps;

    // Sync NodeContainer dimensions from JS-measured layout props (__layout*).
    // On Harmony, CSS px ≈ vp, so getBoundingClientRect() values can be used directly.
    // This is critical for auto-height textarea: JS changes this.style.height and
    // immediately calls mountTextarea(), passing the new height via __layoutHeight.
    // NativeEmbedStatus.UPDATE may fire asynchronously (or not at all) for CSS-driven
    // resizes, so we propagate the height here to make NodeContainer follow immediately.
    const layoutHeight = this.extractNumberProp(freshProps, '__layoutHeight');
    const layoutWidth = this.extractNumberProp(freshProps, '__layoutWidth');
    if (layoutWidth !== undefined && layoutWidth > 0) {
      updateParams.width = layoutWidth;
    }
    if (layoutHeight !== undefined && layoutHeight > 0) {
      updateParams.height = layoutHeight;
    }

    nodeController.updateRenderOption(updateParams);

    if (nodeController.getComponentType() === 'video.native') {
      this.applyVideoRotationFromProps(componentId, freshProps);
    }

    this.onComponentsChanged?.();
    return true;
  }

  private applyVideoRotationFromProps(componentId: string, freshProps: NativeComponentRecord): void {
    const rawRotate = freshProps.get('contentRotate');
    const finalRotate = this.parseRotateDegrees(rawRotate);
    const shouldClearRotation = this.shouldClearProp(freshProps, 'contentRotate');
    nativeComponentLog(
      `updateComponentProps(video): componentId=${componentId} clearContentRotate=${shouldClearRotation} contentRotate=${finalRotate}`
    );
    if (shouldClearRotation) {
      LxNativeVideoPlayer.setRotationStatic(componentId, undefined);
    } else if (freshProps.has('contentRotate') && finalRotate !== undefined) {
      LxNativeVideoPlayer.setRotationStatic(componentId, finalRotate);
    }
  }

  private parseRotateDegrees(rawRotate: NativeComponentValue | undefined): number | undefined {
    if (typeof rawRotate === 'number') {
      return Number.isFinite(rawRotate) ? rawRotate : undefined;
    }
    if (typeof rawRotate === 'string') {
      const parsed = Number.parseInt(rawRotate.trim(), 10);
      return Number.isNaN(parsed) ? undefined : parsed;
    }
    if (rawRotate !== null && rawRotate !== undefined) {
      const coerced = Number(rawRotate);
      return Number.isFinite(coerced) ? coerced : undefined;
    }
    return undefined;
  }

  private shouldClearProp(props: NativeComponentRecord, key: string): boolean {
    const raw = props.get('__clearProps');
    if (!Array.isArray(raw)) {
      return false;
    }
    for (const item of raw as NativeComponentValue[]) {
      if (typeof item === 'string' && item === key) {
        return true;
      }
    }
    return false;
  }

  private parseComponentType(embedType: string): string {
    // Convert 'native/video' to 'video.native', 'native/component' to 'component.native'
    if (embedType.startsWith('native/')) {
      const typeName = embedType.substring(7);
      return `${typeName}.native`;
    }
    return embedType;
  }

  private readEmbedInfoNumber(embed: NativeEmbedDataInfo, key: string): number {
    const embedData = embed as EmbedDataLike | undefined;
    let raw: NativeComponentValue | undefined;
    if (embedData) {
      if (key === 'x') {
        raw = embedData.x;
      } else if (key === 'y') {
        raw = embedData.y;
      } else if (key === 'width') {
        raw = embedData.width;
      } else if (key === 'height') {
        raw = embedData.height;
      }
    }
    if (raw !== undefined && raw !== null) {
      return this.normalizeEmbedNumber(raw);
    }
    const info = embedData?.info as EmbedInfoLike | undefined;
    if (!info) {
      return 0;
    }
    if (typeof info.get === 'function') {
      if (key === 'x') {
        raw = info.get('x');
      } else if (key === 'y') {
        raw = info.get('y');
      } else if (key === 'width') {
        raw = info.get('width');
      } else if (key === 'height') {
        raw = info.get('height');
      }
    }
    if (raw === undefined || raw === null) {
      if (key === 'x') {
        raw = info.x;
      } else if (key === 'y') {
        raw = info.y;
      } else if (key === 'width') {
        raw = info.width;
      } else if (key === 'height') {
        raw = info.height;
      }
    }
    return this.normalizeEmbedNumber(raw);
  }

  private extractNumberProp(props: NativeComponentRecord, key: string): number | undefined {
    const raw = props.get(key);
    if (typeof raw === 'number' && Number.isFinite(raw)) {
      return raw;
    }
    if (typeof raw === 'string') {
      const parsed = Number(raw);
      if (Number.isFinite(parsed)) {
        return parsed;
      }
    }
    return undefined;
  }

  private normalizeEmbedNumber(raw: NativeComponentValue | undefined): number {
    if (raw === undefined || raw === null) {
      return 0;
    }
    if (typeof raw === 'number') {
      return Number.isFinite(raw) ? raw : 0;
    }
    if (typeof raw === 'string') {
      const parsed = Number(raw);
      return Number.isFinite(parsed) ? parsed : 0;
    }
    return 0;
  }

  private parseEmbedProps(embed: NativeEmbedDataInfo): NativeComponentRecord {
    const props = new NativeComponentRecord();
    interface EmbedInfo {
      get?: (key: string) => string | undefined;
      src?: string;
    }

    const embedData = embed as EmbedDataLike | undefined;
    const info = embedData?.info as EmbedInfo | undefined;
    let rawProps: string | undefined;
    if (info) {
      if (typeof info.get === 'function') {
        const fromGet = info.get('src');
        if (typeof fromGet === 'string') {
          rawProps = fromGet;
        }
      }
      if (!rawProps && typeof info.src === 'string') {
        rawProps = info.src;
      }
    }
    if (!rawProps && typeof embedData?.src === 'string') {
      rawProps = embedData.src;
    }
    const propsString: string = rawProps ?? '';
    const parsed = propsString ? this.decodeProps(propsString) : null;
    if (parsed) {
      parsed.forEach((key, value) => {
        props.set(key, value);
      });
    }

    return props;
  }

  private decodeProps(raw: string): NativeComponentRecord | null {
    if (!raw) {
      return null;
    }
    try {
      let normalized = raw;
      if (raw.startsWith(PROPS_DATA_PREFIX)) {
        normalized = decodeURIComponent(raw.substring(PROPS_DATA_PREFIX.length));
      }
      if (normalized.startsWith('{')) {
        const parsed: ESObject = JSON.parse(normalized) as ESObject;
        return NativeComponentRecord.fromESObject(parsed);
      }
    } catch (e) {
      nativeComponentError(`Failed to parse embed props: ${e}`);
    }
    return null;
  }

  hasComponent(componentId: string): boolean {
    return this.nodeControllerMap.has(componentId);
  }

  dispatchComponentEvent(componentId: string, eventName: string, detail: NativeComponentValue): void {
    const normalizedEventName = eventName.trim().toLowerCase();
    if (!normalizedEventName) {
      return;
    }
    const nodeController = this.nodeControllerMap.get(componentId);
    const componentType = nodeController?.getComponentType() ?? '';
    if (normalizedEventName === 'focus') {
      this.focusedComponentId = componentId;
      this.focusedComponentType = componentType;
      this.focusedAdjustPosition = this.componentAdjustPosition.get(componentId) ?? true;
    } else if (normalizedEventName === 'blur' && this.focusedComponentId === componentId) {
      this.focusedComponentId = '';
      this.focusedComponentType = '';
      this.focusedAdjustPosition = true;
    }
    const payload = new NativeComponentRecord();
    payload.set('action', 'component.event');
    payload.set('id', componentId);
    payload.set('componentId', componentId);
    payload.set('event', normalizedEventName);
    payload.set('detail', detail ?? new NativeComponentEmptyDetail());
    this.updatePlaybackIntent(componentId, normalizedEventName);
    this.eventDispatcher?.(payload);
    this.dispatchPageFunc(componentId, payload);
  }

  private parseObjectRecord(raw: NativeComponentValue | undefined): NativeComponentRecord | null {
    if (!raw || typeof raw !== 'object' || Array.isArray(raw)) {
      return null;
    }
    const plain = raw as Record<string, NativeComponentValue>;
    const record = new NativeComponentRecord();
    let count = 0;
    Object.keys(plain).forEach((key: string) => {
      const normalized = key.trim();
      if (!normalized) {
        return;
      }
      record.set(normalized, plain[key]);
      count += 1;
    });
    return count > 0 ? record : null;
  }

  private parseObjectRecordFromJson(raw: NativeComponentValue | undefined): NativeComponentRecord | null {
    if (typeof raw !== 'string') {
      return null;
    }
    try {
      const parsed: ESObject = JSON.parse(raw) as ESObject;
      return this.parseObjectRecord(parsed as Record<string, NativeComponentValue>);
    } catch (_error) {
      return null;
    }
  }

  private updatePageFuncBindings(componentId: string, props: NativeComponentRecord): void {
    if (!props.has('pageFuncBindings') && !props.has('pageFuncBindingsJson')) {
      return;
    }
    const rawRecord =
      this.parseObjectRecord(props.get('pageFuncBindings')) ??
      this.parseObjectRecordFromJson(props.get('pageFuncBindingsJson'));
    if (!rawRecord) {
      this.componentPageFuncBindings.delete(componentId);
      return;
    }
    const next = new Map<string, string>();
    rawRecord.forEach((eventName: string, rawValue: NativeComponentValue) => {
      const eventKey = eventName.trim().toLowerCase();
      if (!eventKey) {
        return;
      }
      if (typeof rawValue !== 'string') {
        return;
      }
      const fnName = rawValue.trim();
      if (!fnName) {
        return;
      }
      next.set(eventKey, fnName);
    });
    if (next.size === 0) {
      this.componentPageFuncBindings.delete(componentId);
      return;
    }
    this.componentPageFuncBindings.set(componentId, next);
  }

  private updateComponentDataset(componentId: string, props: NativeComponentRecord): void {
    if (!props.has('dataset') && !props.has('datasetJson')) {
      return;
    }
    const next =
      this.parseObjectRecord(props.get('dataset')) ??
      this.parseObjectRecordFromJson(props.get('datasetJson'));
    if (!next) {
      this.componentDataset.delete(componentId);
      return;
    }
    this.componentDataset.set(componentId, next);
  }

  private parseWebTagRoute(): WebTagRoute | null {
    const logicalTag = (this.webTag.split('#')[0] || this.webTag).trim();
    const separator = logicalTag.indexOf(':');
    if (separator <= 0 || separator >= logicalTag.length - 1) {
      return null;
    }
    const appId = logicalTag.substring(0, separator);
    const path = logicalTag.substring(separator + 1);
    if (!appId || !path) {
      return null;
    }
    const route = new WebTagRoute();
    route.appId = appId;
    route.path = path;
    return route;
  }

  private dispatchPageFunc(componentId: string, payload: NativeComponentRecord): void {
    const eventValue = payload.get('event');
    if (typeof eventValue !== 'string') {
      return;
    }
    const eventName = eventValue.trim().toLowerCase();
    if (!eventName) {
      return;
    }
    const bindings = this.componentPageFuncBindings.get(componentId);
    if (!bindings || bindings.size === 0) {
      return;
    }
    const boundFunctionName = bindings.get(eventName);
    if (!boundFunctionName) {
      return;
    }
    const route = this.parseWebTagRoute();
    if (!route) {
      nativeComponentError(`drop nativecomponent event: invalid webTag=${this.webTag} componentId=${componentId}`);
      return;
    }
    const pageEvent = new NativeComponentRecord();
    const detail = payload.has('detail') ? payload.get('detail') : new NativeComponentEmptyDetail();
    const dataset = this.componentDataset.get(componentId) ?? new NativeComponentRecord();
    const target = new NativeComponentRecord();
    target.set('id', componentId);
    target.set('dataset', dataset);
    pageEvent.set('type', eventName);
    pageEvent.set('detail', detail);
    pageEvent.set('id', componentId);
    pageEvent.set('dataset', dataset);
    pageEvent.set('target', target);
    pageEvent.set('currentTarget', target);
    pageEvent.set('timeStamp', Date.now());

    const bindingsPayload = new NativeComponentRecord();
    bindingsPayload.set(eventName, boundFunctionName);

    const accepted: boolean = onNativeComponentEvent(
      route.appId,
      route.path,
      componentId,
      eventName,
      pageEvent.toJSONString(),
      bindingsPayload.toJSONString()
    );
    if (!accepted) {
      nativeComponentError(`drop nativecomponent event: runtime rejected appid=${route.appId} path=${route.path} componentId=${componentId} event=${eventName}`);
    }
  }


  /**
   * Pause all native components (e.g., on page navigate away)
   */
  pauseAll(): void {
    this.lifecyclePaused = true;
    this.componentIdArr.forEach((componentId) => {
      const nodeController = this.nodeControllerMap.get(componentId);
      if (nodeController) {
        const intentPlaying = this.componentPlaybackIntent.get(componentId) === true;
        // Prefer real runtime state for video components; event intent can be stale
        // when page lifecycle changes race with bridge event delivery.
        const snapshot = nodeController.getComponentType() === 'video.native'
          ? LxNativeVideoPlayer.getPlaybackState(componentId)
          : undefined;
        const runtimeShouldResume =
          snapshot?.isPlaying === true ||
          (snapshot?.isBuffering === true && snapshot?.isPausedByUser !== true);
        const shouldAutoResume = this.componentsPendingAutoResume.has(componentId) || intentPlaying || runtimeShouldResume;
        if (shouldAutoResume) {
          this.componentsPendingAutoResume.add(componentId);
        } else {
          this.componentsPendingAutoResume.delete(componentId);
        }
        // Send pause command via updateNode - component watches for __command
        nodeController.updateNode(new ComponentCommand('pause'));
      }
    });
    // Intentionally quiet; managed by caller visibility handling
  }

  /**
   * Resume all native components (e.g., on page navigate back)
   */
  resumeAll(): void {
    if (!this.lifecyclePaused) {
      return;
    }
    this.lifecyclePaused = false;
    this.componentIdArr.forEach((componentId) => {
      const nodeController = this.nodeControllerMap.get(componentId);
      if (nodeController) {
        // Send resume command to clear pending pause
        nodeController.updateNode(new ComponentCommand('resume'));
        if (this.componentsPendingAutoResume.has(componentId)) {
          this.componentsPendingAutoResume.delete(componentId);
          nodeController.updateNode(new ComponentCommand('play'));
        }
      }
    });
    // Intentionally quiet; managed by caller visibility handling
  }

  /**
   * Stop all native components for this webtag to release decoder/runtime resources.
   */
  stopAll(): void {
    this.componentIdArr.forEach((componentId) => {
      const nodeController = this.nodeControllerMap.get(componentId);
      if (nodeController) {
        nodeController.updateNode(new ComponentCommand('stop'));
      }
    });
  }

  /**
   * Cleanup all resources
   */
  teardown(): void {
    this.lifecyclePaused = false;
    this.focusedComponentId = '';
    this.focusedComponentType = '';
    this.focusedAdjustPosition = true;
    this.pendingUnmountTimers.forEach((timerId) => {
      clearTimeout(timerId);
    });
    this.pendingUnmountTimers.clear();
    this.componentPlaybackIntent.clear();
    this.componentAdjustPosition.clear();
    this.componentsPendingAutoResume.clear();
    this.pendingPropsByComponentId.clear();

    this.componentIdArr.forEach((componentId) => {
      const nodeController = this.nodeControllerMap.get(componentId);
      if (nodeController?.getComponentType() === 'video.native') {
        LxNativeVideoPlayer.destroyStatic(componentId);
      }
      const callbackId = getComponentCallback(componentId);
      if (callbackId) {
        const payload = new NativeComponentRecord();
        payload.set('action', 'component.event');
        payload.set('id', componentId);
        payload.set('componentId', componentId);
        payload.set('event', 'unmount');
        payload.set('detail', new NativeComponentEmptyDetail());
        onCallback(callbackId, true, payload.toJSONString());
        removeComponentCallback(componentId);
      }
      if (nodeController) {
        nodeController.setBuilderNode(null);
        nodeController.rebuild();
      }
    });
    this.nodeControllerMap.clear();
    this.componentIdArr = [];
    this.componentPageFuncBindings.clear();
    this.componentDataset.clear();
  }

  private readBooleanProp(props: NativeComponentRecord, key: string, fallback: boolean): boolean {
    const raw = props.get(key);
    if (typeof raw === 'boolean') {
      return raw;
    }
    if (typeof raw === 'number') {
      return raw !== 0;
    }
    if (typeof raw === 'string') {
      const normalized = raw.trim().toLowerCase();
      if (normalized === 'true' || normalized === '1') return true;
      if (normalized === 'false' || normalized === '0') return false;
    }
    return fallback;
  }

  private updateComponentAdjustPosition(componentId: string, props: NativeComponentRecord): void {
    if (!props) {
      return;
    }
    const adjustPosition = this.readBooleanProp(
      props,
      'adjustPosition',
      this.readBooleanProp(props, 'adjust-position', true)
    );
    this.componentAdjustPosition.set(componentId, adjustPosition);
    if (this.focusedComponentId === componentId) {
      this.focusedAdjustPosition = adjustPosition;
    }
  }

  private updatePlaybackIntent(componentId: string, eventValue: NativeComponentValue | undefined): void {
    if (typeof eventValue !== 'string') {
      return;
    }
    if (eventValue === 'play' || eventValue === 'playrequest' || eventValue === 'playing') {
      this.componentPlaybackIntent.set(componentId, true);
      return;
    }
    if (eventValue === 'pause' || eventValue === 'stop' || eventValue === 'ended' || eventValue === 'error') {
      this.componentPlaybackIntent.set(componentId, false);
    }
  }
}
