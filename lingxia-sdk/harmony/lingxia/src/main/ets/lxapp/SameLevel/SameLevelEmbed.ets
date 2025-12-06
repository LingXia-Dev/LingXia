/**
 * SameLevelEmbed - Embed native components in WebView using official ArkWeb pattern
 * This module provides the embed handlers for Web component
 */
import { NodeRenderType } from '@kit.ArkUI';
import { webview } from '@kit.ArkWeb';
import { sameLevelError } from './SameLevelTypes';
import { SameLevelNodeController, NodeControllerParams } from './SameLevelNodeController';

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
  private eventDispatcher?: (payload: Record<string, Object>) => void;

  constructor(webTag: string, eventDispatcher?: (payload: Record<string, Object>) => void) {
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
  setEventDispatcher(dispatcher?: (payload: Record<string, Object>) => void): void {
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
      props['__eventSink'] = eventSink as Object;
    }

    const widthPx = embed.info?.width || 0;
    const heightPx = embed.info?.height || 0;
    const widthVp = this.uiContext?.px2vp(widthPx) || 0;
    const heightVp = this.uiContext?.px2vp(heightPx) || 0;

    const params: NodeControllerParams = {
      surfaceId: embed.surfaceId || '',
      type: componentType,
      renderType: NodeRenderType.RENDER_TYPE_TEXTURE,
      embedId: embed.embedId || '',
      width: widthVp,
      height: heightVp,
      componentId: componentId,
      props: props
    };

    nodeController.setRenderOption(params);

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
    const nodeController = this.nodeControllerMap.get(componentId);
    if (!nodeController) return;

    const props = this.parseEmbedProps(embed);
    const eventSink = this.buildEventSink(componentId);
    if (eventSink) {
      props['__eventSink'] = eventSink as Object;
    }

    // Update layout sizes (vp) when embed dimensions change
    const widthPx = embed.info?.width || 0;
    const heightPx = embed.info?.height || 0;
    const widthVp = this.uiContext?.px2vp(widthPx) || 0;
    const heightVp = this.uiContext?.px2vp(heightPx) || 0;

    nodeController.updateRenderOption({
      width: widthVp,
      height: heightVp,
      props: props
    });

    // Notify listeners so NodeContainer layout refreshes
    this.onComponentsChanged?.();
  }

  private handleDestroy(componentId: string): void {
    const nodeController = this.nodeControllerMap.get(componentId);
    if (!nodeController) return;

    nodeController.setBuilderNode(null);
    nodeController.rebuild();
    
    this.nodeControllerMap.delete(componentId);
    const index = this.componentIdArr.indexOf(componentId);
    if (index > -1) {
      this.componentIdArr.splice(index, 1);
    }

    this.onComponentsChanged?.();
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

  private parseEmbedProps(embed: NativeEmbedDataInfo): Record<string, Object> {
    // Parse props from embed info - implementation depends on how props are passed
    // This could be from data attributes or tag content
    const props: Record<string, Object> = {};
    const info = embed.info as Record<string, Object> | undefined;

    // Add basic info as props
    if (info && info['tag']) {
      props['tag'] = info['tag'] as Object;
    }

    const rawProps = info ? this.extractPropsString(info) : '';
    const parsed = rawProps ? this.decodeProps(rawProps) : null;
    if (parsed) {
      const parsedKeys = Object.keys(parsed);
      for (let i = 0; i < parsedKeys.length; i++) {
        const key = parsedKeys[i];
        props[key] = parsed[key];
      }
    }

    return props;
  }

  private extractPropsString(info: Record<string, Object>): string {
    const rawSrc = info['src'];
    if (typeof rawSrc === 'string' && rawSrc.length > 0) {
      return rawSrc;
    }
    const dataLxProps = info['data-lx-props'];
    if (typeof dataLxProps === 'string' && dataLxProps.length > 0) {
      return dataLxProps;
    }
    const dataLxPropsAlt = info['dataLxProps'];
    if (typeof dataLxPropsAlt === 'string' && dataLxPropsAlt.length > 0) {
      return dataLxPropsAlt;
    }
    return '';
  }

  private decodeProps(raw: string): Record<string, Object> | null {
    if (!raw) {
      return null;
    }
    try {
      let normalized = raw;
      if (raw.startsWith(PROPS_DATA_PREFIX)) {
        normalized = decodeURIComponent(raw.substring(PROPS_DATA_PREFIX.length));
      }
      if (normalized.startsWith('{') || normalized.startsWith('[')) {
        return JSON.parse(normalized) as Record<string, Object>;
      }
    } catch (e) {
      sameLevelError(`Failed to parse embed props: ${e}`);
    }
    return null;
  }

  private buildEventSink(componentId: string): ((event: Record<string, Object>) => void) | undefined {
    if (!this.eventDispatcher) {
      return undefined;
    }
    return (event: Record<string, Object>) => {
      const payload: Record<string, Object> = new Object() as Record<string, Object>;
      if (event) {
        Object.keys(event).forEach((key: string) => {
          payload[key] = event[key];
        });
      }
      payload['action'] = 'component.event';
      payload['id'] = componentId;
      if (!payload['componentId']) {
        payload['componentId'] = componentId;
      }
      this.eventDispatcher!(payload);
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
