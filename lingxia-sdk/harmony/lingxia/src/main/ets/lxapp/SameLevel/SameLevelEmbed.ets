/**
 * SameLevelEmbed - Embed native components in WebView using official ArkWeb pattern
 * This module provides the embed handlers for Web component
 */
import { NodeRenderType } from '@kit.ArkUI';
import { webview } from '@kit.ArkWeb';
import { SameLevelEmptyDetail, SameLevelRecord, sameLevelError, sameLevelLog } from './SameLevelTypes';
import { getComponentCallback, removeComponentCallback } from './SameLevelCallbackRegistry';
import { onCallback } from 'liblingxia.so';
import { SameLevelNodeController, NodeControllerParams, NodeControllerUpdateParams } from './SameLevelNodeController';
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

/**
 * Manager for handling same-level embedding in a Web component
 */
export class SameLevelEmbedManager {
  private webTag: string;
  private nodeControllerMap: Map<string, SameLevelNodeController> = new Map();
  private componentIdArr: string[] = [];
  private uiContext: UIContext | null = null;
  private onComponentsChanged?: () => void;
  private eventDispatcher?: (payload: SameLevelRecord) => void;
  private pendingUnmountTimers: Map<string, number> = new Map();

  constructor(webTag: string, eventDispatcher?: (payload: SameLevelRecord) => void) {
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

  /**
   * Set dispatcher used to forward native events back to JavaScript
   */
  setEventDispatcher(dispatcher?: (payload: SameLevelRecord) => void): void {
    this.eventDispatcher = dispatcher;
  }

  /**
   * Get current component ID array (for ForEach rendering)
   */
  getComponentIds(): string[] {
    return this.componentIdArr;
  }

  /**
   * Get NodeController for a component ID
   */
  getNodeController(componentId: string): SameLevelNodeController | undefined {
    return this.nodeControllerMap.get(componentId);
  }

  /**
   * Handle native embed lifecycle change event from Web component
   * This should be called from Web.onNativeEmbedLifecycleChange
   */
  handleLifecycleChange(embed: NativeEmbedDataInfo): void {
    const componentId = embed.info?.id?.toString() || '';
    if (!componentId) {
      sameLevelError('Embed lifecycle event missing component ID');
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

    const nodeController = new SameLevelNodeController();

    // Parse component type from embed type (e.g., 'native/video' -> 'video.native')
    const embedType = embed.info?.type || 'native/component';
    const componentType = this.parseComponentType(embedType);

    // Get props from embed tag attribute
    const props = this.parseEmbedProps(embed);
    const eventSink = this.buildEventSink(componentId);
    if (eventSink) {
      props.set('__eventSink', eventSink as object);
    }

    const widthPx = embed.info?.width || 0;
    const heightPx = embed.info?.height || 0;
    const widthVp = this.uiContext?.px2vp(widthPx) || 0;
    const heightVp = this.uiContext?.px2vp(heightPx) || 0;

    const params = new NodeControllerParams();
    params.surfaceId = embed.surfaceId || '';
    params.type = componentType;
    // Use TEXTURE render type to preserve ArkWeb same-level semantics and coordinate mapping.
    // (DISPLAY may break overlay/layout for same-level components.)
    params.renderType = NodeRenderType.RENDER_TYPE_TEXTURE;
    params.embedId = embed.embedId || '';
    params.width = widthVp;
    params.height = heightVp;
    params.componentId = componentId;
    params.props = props;

    nodeController.setRenderOption(params);
    if (componentType === 'video.native') {
      sameLevelLog(
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

    const props = this.parseEmbedProps(embed);
    const eventSink = this.buildEventSink(componentId);
    if (eventSink) {
      props.set('__eventSink', eventSink as object);
    }

    // Update layout sizes (vp) when embed dimensions change
    const widthPx = embed.info?.width || 0;
    const heightPx = embed.info?.height || 0;
    const widthVp = this.uiContext?.px2vp(widthPx) || 0;
    const heightVp = this.uiContext?.px2vp(heightPx) || 0;

    const updateParams = new NodeControllerUpdateParams();
    updateParams.width = widthVp;
    updateParams.height = heightVp;
    updateParams.props = props;
    nodeController.updateRenderOption(updateParams);
    if (nodeController.getComponentType() === 'video.native') {
      sameLevelLog(`embed update(video): componentId=${componentId} embedId=${embed.embedId || ''} size=${widthVp}x${heightVp}`);
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
      sameLevelLog(`embed destroy(video): componentId=${componentId}`);
    }

    nodeController.setBuilderNode(null);
    nodeController.rebuild();

    this.nodeControllerMap.delete(componentId);
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

      const payload = new SameLevelRecord();
      payload.set('action', 'component.event');
      payload.set('id', componentId);
      payload.set('componentId', componentId);
      payload.set('event', 'unmount');
      payload.set('detail', new SameLevelEmptyDetail());
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
    this.componentIdArr.forEach((componentId) => {
      const nodeController = this.nodeControllerMap.get(componentId);
      if (nodeController && nodeController.getEmbedId() === touch.embedId) {
        nodeController.postEvent(touch.touchEvent);
      }
    });
  }

  private parseComponentType(embedType: string): string {
    // Convert 'native/video' to 'video.native', 'native/component' to 'component.native'
    if (embedType.startsWith('native/')) {
      const typeName = embedType.substring(7);
      return `${typeName}.native`;
    }
    return embedType;
  }

  private parseEmbedProps(embed: NativeEmbedDataInfo): SameLevelRecord {
    const props = new SameLevelRecord();
    interface EmbedInfo {
      get?: (key: string) => string | undefined;
      src?: string;
    }

    const info = embed.info as EmbedInfo | undefined;
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
    const propsString: string = rawProps ?? '';
    const parsed = propsString ? this.decodeProps(propsString) : null;
    if (parsed) {
      parsed.forEach((key, value) => {
        props.set(key, value);
      });
    }

    return props;
  }

  private decodeProps(raw: string): SameLevelRecord | null {
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
        return SameLevelRecord.fromESObject(parsed);
      }
    } catch (e) {
      sameLevelError(`Failed to parse embed props: ${e}`);
    }
    return null;
  }

  private buildEventSink(componentId: string): ((event: SameLevelRecord) => void) | undefined {
    if (!this.eventDispatcher) {
      return undefined;
    }
    return (event: SameLevelRecord) => {
      const payload = new SameLevelRecord();
      event.forEach((key, value) => {
        payload.set(key, value);
      });
      payload.set('action', 'component.event');
      payload.set('id', componentId);
      payload.set('componentId', componentId);
      this.eventDispatcher?.(payload);
    };
  }


  /**
   * Pause all native components (e.g., on page navigate away)
   */
  pauseAll(): void {
    this.componentIdArr.forEach((componentId) => {
      const nodeController = this.nodeControllerMap.get(componentId);
      if (nodeController) {
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
    this.componentIdArr.forEach((componentId) => {
      const nodeController = this.nodeControllerMap.get(componentId);
      if (nodeController) {
        // Send resume command to clear pending pause
        nodeController.updateNode(new ComponentCommand('resume'));
      }
    });
    // Intentionally quiet; managed by caller visibility handling
  }

  /**
   * Cleanup all resources
   */
  teardown(): void {
    this.componentIdArr.forEach((componentId) => {
      const nodeController = this.nodeControllerMap.get(componentId);
      if (nodeController) {
        nodeController.setBuilderNode(null);
        nodeController.rebuild();
      }
    });
    this.nodeControllerMap.clear();
    this.componentIdArr = [];
  }
}
