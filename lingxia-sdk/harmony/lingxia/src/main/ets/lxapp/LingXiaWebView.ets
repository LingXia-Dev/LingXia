/**
 * LingXiaWebView - Self-contained WebView component for same-level rendering
 *
 * Note:
 * - WebView registry/lifecycle core is sourced from `lingxia-webview/src/harmony/arkts/WebViewCore.ets`
 *   and synced into this module during SDK packaging.
 * - This component remains in SDK because it binds SDK-specific NativeComponents.
 *
 * Owns all WebView-specific concerns:
 * - Web component declaration with controller
 * - Security profile config (domStorage, fileAccess, database, mixedMode)
 * - Navigation policy (onLoadIntercept)
 * - Controller lifecycle (onAppear → onWebviewControllerCreated)
 * - Same-level native embed (enableNativeEmbedMode, lifecycle, gesture)
 * - Embed manager setup/cleanup/sync
 * - NodeContainer rendering for same-level components
 * - Fullscreen detection → notifies container via callback
 */
import { hilog } from '@kit.PerformanceAnalysisKit';
import { webview } from '@kit.ArkWeb';
import {
  getWebViewEmbedManager,
  handleWebViewDownloadStart,
  handleWebViewNavigationPolicy,
  isBrowserRelaxed as isWebViewBrowserRelaxed,
  notifyWebViewControllerAttached
} from './WebViewBridge';
import { NativeComponentEmbedManager } from './NativeComponents/NativeComponentEmbed';
import { setEmbedController, teardownEmbedManager } from './NativeComponents/NativeBridgeInternal';

const DOMAIN = 0x0000;
const TAG = 'LingXia.WebView';

// Layout snapshot for same-level NodeContainer
interface NodeLayoutState {
  width: Length;
  height: Length;
  position?: Position;
  zIndex: number;
  cornerRadius: number;
}

interface ApplyNodeLayoutsResult {
  changed: boolean;
  nextMap: Map<string, NodeLayoutState>;
}

@Component
export struct LingXiaWebView {
  @Prop webTag: string = ''
  controller: webview.WebviewController = new webview.WebviewController()
  onFullscreenChange?: (webTag: string, componentId: string, isFullscreen: boolean) => void

  // Same-level rendering state (simplified: single webTag, no Map wrappers)
  @State private embedComponentIds: string[] = []
  @State private nodeLayoutCache: Map<string, NodeLayoutState> = new Map()
  @State private fullscreenRefreshKey: number = 0
  private embedManager: NativeComponentEmbedManager | undefined
  private controllerAcked: boolean = false
  private embedSyncScheduled: boolean = false
  private embedSyncTimerId: number | null = null
  private lastFullscreenId: string = ''

  aboutToDisappear() {
    this.cleanupEmbedManager();
  }

  build() {
    Stack() {
      // Same-level native components (rendered via NodeController, MUST be before Web)
      // Note: We use stable keys (componentId only) to avoid component recreation.
      // ArkUI doesn't allow reusing the same FrameNode when key changes.
      // Layout updates are handled through nodeLayoutCache @State variable.
      ForEach(this.embedComponentIds, (componentId: string) => {
        this.buildNodeContainer(componentId, this.fullscreenRefreshKey)
      }, (componentId: string) => componentId)

      // Web component with WebView-specific config only
      Web({ src: '', controller: this.controller })
        .onLoadIntercept((event) => {
          if (event && event.data) {
            const url = event.data.getRequestUrl();
            if (url) {
              return handleWebViewNavigationPolicy(this.webTag, url);
            }
          }
          return false;
        })
        .onDownloadStart((event) => {
          if (!event) {
            return;
          }
          handleWebViewDownloadStart(
            this.webTag,
            event.url ?? '',
            event.userAgent ?? '',
            event.contentDisposition ?? '',
            event.mimetype ?? '',
            event.contentLength ?? -1
          );
        })
        .onAppear(() => {
          hilog.info(DOMAIN, TAG, `WebView component appeared for webTag: ${this.webTag}`);
          if (!this.controllerAcked) {
            notifyWebViewControllerAttached(this.webTag);
            this.controllerAcked = true;
          }
        })
        .onControllerAttached(() => {
          this.setupEmbedManager();
          hilog.info(DOMAIN, TAG, `Same-level embed manager setup for webTag: ${this.webTag}`);
        })
        .onDisAppear(() => {
          this.cleanupEmbedManager();
          hilog.info(DOMAIN, TAG, `Same-level cleanup for webTag: ${this.webTag}`);
        })
        // Enable native embed mode for same-level rendering
        .enableNativeEmbedMode(true)
        .onNativeEmbedLifecycleChange((embed: NativeEmbedDataInfo) => {
          if (this.embedManager) {
            this.embedManager.setUIContext(this.getUIContext());
            this.embedManager.handleLifecycleChange(embed);
          }
        })
        .onNativeEmbedGestureEvent((touch: NativeEmbedTouchInfo) => {
          if (this.embedManager) {
            this.embedManager.handleGestureEvent(touch);
          }
        })
        .domStorageAccess(this.isBrowserRelaxed())
        .fileAccess(false)
        .databaseAccess(this.isBrowserRelaxed())
        .mixedMode(MixedMode.None)
        .backgroundColor(Color.Transparent)
        .width('100%')
        .height('100%')
    }
    .width('100%')
    .height('100%')
  }

  // ============================================================================
  // Profile Resolution
  // ============================================================================

  private isBrowserRelaxed(): boolean {
    return isWebViewBrowserRelaxed(this.webTag);
  }

  // ============================================================================
  // Embed Manager Lifecycle
  // ============================================================================

  private setupEmbedManager(): void {
    const manager = getWebViewEmbedManager(this.webTag);
    this.embedManager = manager;
    manager.setUIContext(this.getUIContext());
    manager.setOnComponentsChanged(() => {
      // Defer and batch state writes to avoid mutating @State during render.
      this.scheduleEmbedManagerSync();
    });
    this.scheduleEmbedManagerSync();
    setEmbedController(this.webTag, this.controller);
  }

  /**
   * Cleanup embed manager. Safe to call multiple times.
   */
  private cleanupEmbedManager(): void {
    if (!this.embedManager) {
      return; // Already cleaned up
    }
    if (this.embedSyncTimerId !== null) {
      clearTimeout(this.embedSyncTimerId);
      this.embedSyncTimerId = null;
      this.embedSyncScheduled = false;
    }
    teardownEmbedManager(this.webTag);
    this.embedManager = undefined;
    if (this.embedComponentIds.length > 0) {
      this.embedComponentIds = [];
    }
    if (this.nodeLayoutCache.size > 0) {
      this.nodeLayoutCache = new Map();
    }
    // Notify container if fullscreen was active
    if (this.lastFullscreenId.length > 0) {
      this.lastFullscreenId = '';
      this.onFullscreenChange?.(this.webTag, '', false);
    }
  }

  /**
   * Batch and defer same-level state sync.
   * Avoids @State writes during render and reduces recompose churn while scrolling.
   */
  private scheduleEmbedManagerSync(): void {
    if (this.embedSyncScheduled) {
      return;
    }
    this.embedSyncScheduled = true;
    this.embedSyncTimerId = setTimeout(() => {
      this.embedSyncTimerId = null;
      this.embedSyncScheduled = false;

      if (!this.embedManager) {
        return;
      }

      // Update component IDs
      const nextIds = this.embedManager.getComponentIds().slice();
      let idsChanged = false;
      if (!this.areStringArraysEqual(this.embedComponentIds, nextIds)) {
        this.embedComponentIds = nextIds;
        idsChanged = true;
      }

      // Update layouts
      const layoutResult = this.applyNodeLayouts(this.nodeLayoutCache);
      if (layoutResult.changed) {
        this.nodeLayoutCache = layoutResult.nextMap;
      }

      // Check fullscreen state change
      let fullscreenId = '';
      const componentIds = this.embedManager.getComponentIds();
      for (let i = 0; i < componentIds.length; i++) {
        const id = componentIds[i];
        const controller = this.embedManager.getNodeController(id);
        if (controller?.getIsFullscreen()) {
          fullscreenId = id;
          break;
        }
      }

      if (fullscreenId !== this.lastFullscreenId) {
        this.lastFullscreenId = fullscreenId;
        this.onFullscreenChange?.(this.webTag, fullscreenId, fullscreenId.length > 0);
      }

      if (idsChanged || layoutResult.changed) {
        this.fullscreenRefreshKey++;
      }
    }, 0);
  }

  // ============================================================================
  // Node Layout Management
  // ============================================================================

  /**
   * Compute and cache layout state for all components.
   * This drives NodeContainer attributes without recreating native nodes.
   */
  private applyNodeLayouts(
    source: Map<string, NodeLayoutState>
  ): ApplyNodeLayoutsResult {
    if (!this.embedManager) {
      return { changed: false, nextMap: source };
    }

    let changed = false;
    const next = new Map<string, NodeLayoutState>(source);
    const componentIds = this.embedManager.getComponentIds();

    // Remove stale entries
    const keys = Array.from(next.keys());
    keys.forEach((key: string) => {
      if (!componentIds.includes(key)) {
        next.delete(key);
        changed = true;
      }
    });

    // Update current entries
    componentIds.forEach((componentId: string) => {
      const controller = this.embedManager!.getNodeController(componentId);
      const isFullscreen = controller?.getIsFullscreen() ?? false;
      const normalPosition: Position = {
        x: controller?.getX() ?? 0,
        y: controller?.getY() ?? 0
      };
      const layout: NodeLayoutState = {
        width: isFullscreen ? '100%' : (controller?.getWidth() ?? 0),
        height: isFullscreen ? '100%' : (controller?.getHeight() ?? 0),
        position: isFullscreen ? { x: 0, y: 0 } : normalPosition,
        // Must be higher than backdrop (49999) to appear on top
        // Ensure same-level native nodes are above the Web node in the Stack (Web is rendered later).
        zIndex: isFullscreen ? 50000 : 1,
        cornerRadius: isFullscreen ? 0 : (controller?.getCornerRadius() ?? 0)
      };
      const prev = source.get(componentId);
      const prevPos = prev?.position;
      const nextPos = layout.position;
      const samePosition = (
        prevPos === undefined && nextPos === undefined
      ) || (
        prevPos !== undefined &&
        nextPos !== undefined &&
        prevPos.x === nextPos.x &&
        prevPos.y === nextPos.y
      );
      const sameLayout = prev !== undefined &&
        prev.width === layout.width &&
        prev.height === layout.height &&
        prev.zIndex === layout.zIndex &&
        prev.cornerRadius === layout.cornerRadius &&
        samePosition;
      if (!sameLayout) {
        next.set(componentId, layout);
        changed = true;
      }
    });

    return { changed: changed, nextMap: next };
  }

  private areStringArraysEqual(left: string[], right: string[]): boolean {
    if (left.length !== right.length) {
      return false;
    }
    for (let i = 0; i < left.length; i++) {
      if (left[i] !== right[i]) {
        return false;
      }
    }
    return true;
  }

  private getNodeLayout(componentId: string): NodeLayoutState | undefined {
    return this.nodeLayoutCache.get(componentId);
  }

  // ============================================================================
  // NodeContainer Builder
  // ============================================================================

  @Builder
  buildNodeContainer(componentId: string, refreshKey: number): void {
    // Fullscreen uses separate XComponent in renderFullscreenOverlay(); original stays at same-level position
    if (this.embedManager && this.embedManager.getNodeController(componentId)) {
      NodeContainer(this.embedManager.getNodeController(componentId)!)
        .width(this.getNodeContainerWidth(componentId))
        .height(this.getNodeContainerHeight(componentId))
        .borderRadius(this.getNodeContainerCornerRadius(componentId))
        .clip(true)
        .position(this.getNodeContainerPosition(componentId))
        .zIndex(this.getNodeContainerZIndex(componentId))
        .hitTestBehavior(this.getNodeContainerHitTest(componentId))
    }
  }

  private getNodeContainerCornerRadius(componentId: string): number {
    const controller = this.embedManager?.getNodeController(componentId);
    if (controller) {
      if (controller.getIsFullscreen()) {
        return 0;
      }
      const radius = controller.getCornerRadius();
      if (radius !== undefined) {
        return radius;
      }
    }

    const layout = this.getNodeLayout(componentId);
    return layout?.cornerRadius ?? 0;
  }

  private getNodeContainerWidth(componentId: string): Length {
    const layout = this.getNodeLayout(componentId);
    if (layout) {
      return layout.width;
    }
    const controller = this.embedManager?.getNodeController(componentId);
    return controller?.getWidth() ?? 0;
  }

  private getNodeContainerHeight(componentId: string): Length {
    const layout = this.getNodeLayout(componentId);
    if (layout) {
      return layout.height;
    }
    const controller = this.embedManager?.getNodeController(componentId);
    return controller?.getHeight() ?? 0;
  }

  private getNodeContainerPosition(componentId: string): Position | undefined {
    const layout = this.getNodeLayout(componentId);
    if (layout) {
      return layout.position;
    }
    return undefined;
  }

  private getNodeContainerZIndex(componentId: string): number {
    const layout = this.getNodeLayout(componentId);
    if (layout) {
      return layout.zIndex;
    }
    return 0;
  }

  private getNodeContainerHitTest(componentId: string): HitTestMode {
    // Keep transparent routing for all native components so ArkWeb can
    // resolve touch target based on embed region.
    void componentId;
    return HitTestMode.Transparent;
  }
}
