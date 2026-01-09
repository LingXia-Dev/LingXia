/**
 * NativeComponentNodeController - NodeController implementation for native component rendering
 * Follows official ArkWeb same-level rendering pattern
 *
 * Fullscreen Strategy:
 * - We do NOT call rebuild() when entering/exiting fullscreen because that would destroy
 *   the XComponent and video surface, breaking playback continuity.
 * - Instead, we only update our internal state (isFullscreen, dimensions) and notify
 *   LxAppContainer via the fullscreenCallback.
 * - LxAppContainer handles where to render the NodeContainer:
 *   - Normal mode: rendered in the Web embed position with embed dimensions
 *   - Fullscreen mode: the same NodeContainer is hidden at embed position, and a
 *     fullscreen overlay renders at screen level with 100% width/height
 * - The actual component (e.g., LxNativeVideoPlayer) remains the same instance,
 *   XComponent surface is preserved, and video continues playing seamlessly.
 */
import { BuilderNode, FrameNode, NodeController, NodeRenderType } from '@kit.ArkUI';
import { NativeComponentRecord, NativeComponentValue, nativeComponentError } from './NativeComponentTypes';

/**
 * Parameters for component rendering
 */
export class NativeComponentParams {
  width: number = 0;
  height: number = 0;
  componentId: string = '';
  componentType: string = '';
  props: NativeComponentRecord = new NativeComponentRecord();
}

/**
 * Parameters for NodeController initialization
 */
export class NodeControllerParams {
  surfaceId: string = '';
  type: string = '';
  renderType: NodeRenderType = NodeRenderType.RENDER_TYPE_DISPLAY;
  embedId: string = '';
  width: number = 0;
  height: number = 0;
  componentId: string = '';
  props: NativeComponentRecord = new NativeComponentRecord();
}

/**
 * Parameters for updating render options without rebuilding.
 * Keep this as an explicitly-declared type to satisfy ArkTS lint rules.
 */
export class NodeControllerUpdateParams {
  width?: number;
  height?: number;
  props?: NativeComponentRecord;
}

/**
 * Builder function type for creating components
 */
export type NativeComponentBuilder = (params: NativeComponentParams) => void;

/**
 * Registry of component builders
 */
const componentBuilders: Map<string, WrappedBuilder<[NativeComponentParams]>> = new Map();

/**
 * Register a component builder for a type
 */
export function registerNativeComponentBuilder(type: string, builder: WrappedBuilder<[NativeComponentParams]>): void {
  componentBuilders.set(type, builder);
}

/**
 * NativeComponentNodeController - Controls rendering of native components in WebView
 */
export class NativeComponentNodeController extends NodeController {
  private rootNode: BuilderNode<[NativeComponentParams]> | null = null;
  private embedId: string = '';
  private surfaceId: string = '';
  private renderType: NodeRenderType = NodeRenderType.RENDER_TYPE_DISPLAY;
  private componentWidth: number = 0;
  private componentHeight: number = 0;
  private componentType: string = '';
  private componentId: string = '';
  private props: NativeComponentRecord = new NativeComponentRecord();

  // Fullscreen state - only tracks state, does NOT trigger rebuild
  private isFullscreen: boolean = false;
  private originalWidth: number = 0;
  private originalHeight: number = 0;
  private fullscreenWidth: number = 0;
  private fullscreenHeight: number = 0;
  private fullscreenCallback?: () => void;

  /**
   * Set render options from embed lifecycle event
   */
  setRenderOption(params: NodeControllerParams): void {
    this.surfaceId = params.surfaceId;
    this.renderType = params.renderType;
    this.embedId = params.embedId;
    this.componentWidth = params.width;
    this.componentHeight = params.height;
    this.componentType = params.type;
    this.componentId = params.componentId;
    this.props = params.props;
    // Store original dimensions for fullscreen restore
    this.originalWidth = params.width;
    this.originalHeight = params.height;
  }

  /**
   * Update render options without rebuilding the node
   * Keeps the XComponent surface alive while refreshing layout/props
   */
  updateRenderOption(params: NodeControllerUpdateParams): void {
    if (params.width !== undefined) {
      this.componentWidth = params.width;
      this.originalWidth = params.width;
    }
    if (params.height !== undefined) {
      this.componentHeight = params.height;
      this.originalHeight = params.height;
    }
    if (params.props !== undefined) {
      const incomingProps = params.props || new NativeComponentRecord();
      const nextProps = new NativeComponentRecord();
      this.props.forEach((key: string, value: NativeComponentValue) => {
        nextProps.set(key, value);
      });
      incomingProps.forEach((key: string, value: NativeComponentValue) => {
        nextProps.set(key, value);
      });
      this.props = nextProps;
    }

    // Push updated params into the builder without recreating the node
    if (this.rootNode) {
      const updated = new NativeComponentParams();
      updated.width = this.componentWidth;
      updated.height = this.componentHeight;
      updated.componentId = this.componentId;
      updated.componentType = this.componentType;
      updated.props = this.props;
      this.rootNode.update(updated);
    }
  }

  /**
   * Create the node tree
   *
   * IMPORTANT: If rootNode already exists, we return its FrameNode directly
   * without recreating. This preserves the XComponent surface during fullscreen
   * transitions when the NodeContainer moves between positions.
   */
  makeNode(uiContext: UIContext): FrameNode | null {
    // If we already have a rootNode, reuse it to preserve XComponent surface
    if (this.rootNode) {
      const frameNode = this.rootNode.getFrameNode();
      return frameNode;
    }

    const builder = componentBuilders.get(this.componentType);
    if (!builder) {
      nativeComponentError(`No builder registered for type: ${this.componentType}`);
      return null;
    }

    this.rootNode = new BuilderNode(uiContext, {
      surfaceId: this.surfaceId,
      type: this.renderType
    });

    const initialParams = new NativeComponentParams();
    initialParams.width = this.componentWidth;
    initialParams.height = this.componentHeight;
    initialParams.componentId = this.componentId;
    initialParams.componentType = this.componentType;
    initialParams.props = this.props;
    this.rootNode.build(builder, initialParams);

    return this.rootNode.getFrameNode();
  }

  /**
   * Set builder node (for cleanup)
   */
  setBuilderNode(rootNode: BuilderNode<NativeComponentParams[]> | null): void {
    this.rootNode = rootNode;
  }

  /**
   * Get current builder node
   */
  getBuilderNode(): BuilderNode<[NativeComponentParams]> | null {
    return this.rootNode;
  }

  /**
   * Update node with new parameters
   */
  updateNode(arg: object): void {
    this.rootNode?.update(arg);
  }

  /**
   * Get embed ID
   */
  getEmbedId(): string {
    return this.embedId;
  }

  /**
   * Get component ID
   */
  getComponentId(): string {
    return this.componentId;
  }

  /**
   * Get component type (e.g. video.native)
   */
  getComponentType(): string {
    return this.componentType;
  }

  /**
   * Get original (non-fullscreen) component width (in vp)
   */
  getWidth(): number {
    return this.originalWidth;
  }

  /**
   * Get original (non-fullscreen) component height (in vp)
   */
  getHeight(): number {
    return this.originalHeight;
  }

  /**
   * Get corner radius from props (0 when fullscreen)
   */
  getCornerRadius(): number {
    if (this.isFullscreen) {
      return 0;
    }
    const cornerRadius = this.props.get('cornerRadius');
    return (typeof cornerRadius === 'number') ? cornerRadius : 0;
  }

  /**
   * Post touch event to the node
   */
  postEvent(event: TouchEvent | undefined): boolean {
    return this.rootNode?.postTouchEvent(event) as boolean;
  }

  /**
   * Check if component is in fullscreen mode
   */
  getIsFullscreen(): boolean {
    return this.isFullscreen;
  }

  /**
   * Get the original surface ID (used for fullscreen surface switching)
   */
  getOriginalSurfaceId(): string {
    return this.surfaceId;
  }

  /**
   * Set fullscreen callback
   */
  setFullscreenCallback(callback: () => void): void {
    this.fullscreenCallback = callback;
  }

  /**
   * Enter fullscreen mode
   *
   * IMPORTANT: This method does NOT call rebuild() because that would destroy the
   * XComponent surface and break video playback. Instead, we only update state and
   * notify LxAppContainer to re-render with fullscreen layout attributes.
   *
   * @param screenWidth Screen width in vp
   * @param screenHeight Screen height in vp
   */
  enterFullscreen(screenWidth: number, screenHeight: number): void {
    if (this.isFullscreen) {
      return;
    }

    // Store fullscreen dimensions
    this.fullscreenWidth = screenWidth;
    this.fullscreenHeight = screenHeight;
    this.isFullscreen = true;

    // DO NOT call rebuild() - that destroys the XComponent and breaks video playback
    // The LxAppContainer will handle rendering this NodeContainer at fullscreen size
    // by changing the NodeContainer's layout attributes (width, height, position, zIndex)

    // Notify UI to update layout
    this.fullscreenCallback?.();
  }

  /**
   * Exit fullscreen mode
   *
   * IMPORTANT: This method does NOT call rebuild() to preserve XComponent surface.
   */
  exitFullscreen(): void {
    if (!this.isFullscreen) {
      return;
    }
    // Clear fullscreen state
    this.isFullscreen = false;
    this.fullscreenWidth = 0;
    this.fullscreenHeight = 0;

    // DO NOT call rebuild() - let LxAppContainer handle returning to normal rendering

    // Notify UI to update layout
    if (this.fullscreenCallback) {
      this.fullscreenCallback();
    }
  }

  /**
   * Toggle fullscreen mode
   */
  toggleFullscreen(screenWidth: number, screenHeight: number): void {
    if (this.isFullscreen) {
      this.exitFullscreen();
    } else {
      this.enterFullscreen(screenWidth, screenHeight);
    }
  }
}
