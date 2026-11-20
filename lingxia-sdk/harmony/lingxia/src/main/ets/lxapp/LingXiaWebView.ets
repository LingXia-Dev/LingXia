/**
 * LingXiaWebView - Self-contained WebView component for same-level rendering
 *
 * Note:
 * - WebView registry/lifecycle core and the SDK-specific NativeComponents binding
 *   live together in this Harmony SDK module.
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
import { requestWebFileChooser } from './WebFileChooserBridge';
import {
  embedPortInit,
  getWebViewEmbedManager,
  handleWebViewDownloadStart,
  handleWebViewLoadError,
  handleWebViewNavigationPolicy,
  handleWebViewNewWindowPolicy,
  isEphemeralDataMode,
  notifyWebViewControllerAttached,
  profileRules,
  registerFullScreenExitHandler,
  supportsWebViewNewWindows,
  WebViewManager
} from './WebViewBridge';
import { ProfileRules } from './WebViewCore';
import { NativeComponentEmbedManager } from './NativeComponents/NativeComponentEmbed';
import { setEmbedController, teardownEmbedManager } from './NativeComponents/NativeBridgeInternal';
import { Lingxia } from '../app/Lingxia';
import { LxLog } from '../app/LxLog';
import { PermissionManager } from '../app/PermissionManager';
import {
  checkWebPermissionPolicy,
  notifyWebViewState,
  onWebFullscreenChanged,
  urlCallbackDispatch
} from 'liblingxia.so';
import { isExternalWindowUrl } from './APIs/surface/SurfaceUrlPolicy';

const DOMAIN = 0x0000;
const TAG = 'LingXia.WebView';

function buildEmbedBootstrapScript(initToken: string): string {
  const encodedToken: string = JSON.stringify(initToken);
  return '(function(){\n'
    + '  if (window !== window.top) return;\n'
    + '  var initToken = ' + encodedToken + ';\n'
    + '  var port = null, early = [];\n'
    + '  window.addEventListener(\'message\', function onInit(ev) {\n'
    + '    if (ev && !ev.source && ev.data === initToken && ev.ports && ev.ports.length === 1) {\n'
    + '      ev.stopImmediatePropagation();\n'
    + '      window.removeEventListener(\'message\', onInit, true);\n'
    + '      port = ev.ports[0];\n'
    + '      for (var i = 0; i < early.length; i++) {\n'
    + '        try { port.postMessage(early[i]); } catch (_) {}\n'
    + '      }\n'
    + '      early.length = 0;\n'
    + '    }\n'
    + '  }, true);\n'
    + '  function sendWire(w) {\n'
    + '    if (port) { port.postMessage(w); }\n'
    + '    else if (early.length < 128) { early.push(w); }\n'
    + '  }\n'
    + '  var p = location.protocol;\n'
    + '  if (p !== \'https:\' && p !== \'http:\' && p !== \'lx:\') return;\n'
    + '  if (Object.prototype.hasOwnProperty.call(window, \'lingxia\')) return;\n'
    + '  var enc = typeof TextEncoder === \'function\' ? new TextEncoder() : null;\n'
    + '  function utf8Len(s) {\n'
    + '    if (enc) return enc.encode(s).length;\n'
    + '    return unescape(encodeURIComponent(s)).length;\n'
    + '  }\n'
    + '  var api = Object.freeze({\n'
    + '    postMessage: function(data, options) {\n'
    + '      if (options !== undefined && options !== null && typeof options !== \'object\') {\n'
    + '        throw new TypeError(\'window.lingxia.postMessage: options must be an object\');\n'
    + '      }\n'
    + '      var queue = !!(options && options.queue === true);\n'
    + '      var raw = JSON.stringify(data);\n'
    + '      if (typeof raw !== \'string\') {\n'
    + '        throw new TypeError(\'window.lingxia.postMessage: data must be JSON-serializable\');\n'
    + '      }\n'
    + '      if (utf8Len(raw) > 65536) {\n'
    + '        throw new RangeError(\'window.lingxia.postMessage: payload exceeds 65536 UTF-8 bytes\');\n'
    + '      }\n'
    + '      sendWire((queue ? \'1\' : \'0\') + raw);\n'
    + '    }\n'
    + '  });\n'
    + '  try {\n'
    + '    Object.defineProperty(window, \'lingxia\', { value: api, configurable: false, enumerable: true, writable: false });\n'
    + '  } catch (_) {}\n'
    + '})();\n';
}

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
  @Prop renderVisibility: Visibility = Visibility.Visible
  @Prop renderTranslate: TranslateOptions = { x: 0, y: 0 } as TranslateOptions
  @Prop renderMargin: Margin = { top: 0, bottom: 0, left: 0, right: 0 } as Margin
  @Prop renderWidth: Length = '100%'
  @Prop renderHeight: Length = '100%'
  @Prop renderPadding: Padding = { top: 0, bottom: 0, left: 0, right: 0 } as Padding
  @Prop renderBorderRadius: BorderRadiuses = {
    topLeft: 0,
    topRight: 0,
    bottomLeft: 0,
    bottomRight: 0
  }
  @Prop renderClip: boolean = false
  @Prop renderZIndex: number = 0
  @Prop renderHitTest: HitTestMode = HitTestMode.Default
  @Prop appearanceDark: boolean = false
  @Prop syncRender: boolean = false
  /// URL-callback (login) surfaces: target=_blank / window.open leave the
  /// sheet in place and hand http(s) to the system browser.
  @Prop openNewWindowExternally: boolean = false
  controller: webview.WebviewController = new webview.WebviewController()
  onFullscreenChange?: (webTag: string, componentId: string, isFullscreen: boolean) => void
  /// A GPU page's `<webview>`: true when ArkWeb's own pan should yield a drag of `dx`, `dy` vp.
  yieldsPan?: (dx: number, dy: number) => boolean

  // Same-level rendering state (simplified: single webTag, no Map wrappers)
  @State private embedComponentIds: string[] = []
  @State private nodeLayoutCache: Map<string, NodeLayoutState> = new Map()
  @State private fullscreenRefreshKey: number = 0
  private embedManager: NativeComponentEmbedManager | undefined
  private controllerAcked: boolean = false
  private embedSyncScheduled: boolean = false
  private embedSyncTimerId: number | null = null
  private lastFullscreenId: string = ''
  private pageEpoch: string = ''
  // ArkWeb can skip onPageVisible (splash, zero-size, off-screen tabs),
  // which is the Harmony analogue of Android skipping onPageCommitVisible.
  // Strict lxapp pages still need a MessagePort from onPageEnd in that case.
  private documentCommitted: boolean = false
  // URL this begin reported. native matches a commit against it exactly, so the
  // onPageEnd fallback commits with it rather than with the end callback's URL.
  private pageBeginUrl: string = ''
  // ArkWeb ends error pages too: a load whose main frame failed never commits
  // from its end.
  private mainFrameFailed: boolean = false

  aboutToDisappear() {
    this.cleanupEmbedManager();
  }

  build() {
    Stack() {
      if (this.rules().nativeComponentBridge) {
        ForEach(this.embedComponentIds, (componentId: string) => {
          this.buildNodeContainer(componentId, this.fullscreenRefreshKey)
        }, (componentId: string) => componentId)
      }
      this.buildManagedWeb()
    }
    .width('100%')
    .height('100%')
    // A height-shrunk Web (tabbar reservation) must anchor to the top;
    // Stack's default Center alignment splits the reserved space across
    // top and bottom, opening a band under the navbar.
    .alignContent(Alignment.TopStart)
    // Native islands are Stack siblings of ArkWeb. Keep offscreen and partially
    // visible islands inside the same viewport contract as the page itself.
    .clip(true)
    .translate(this.renderTranslate)
  }

  // ============================================================================
  // Profile Resolution
  // ============================================================================

  private rules(): ProfileRules {
    return profileRules(this.webTag);
  }

  private documentStartScripts(): Array<ScriptItem> {
    if (!this.rules().embedMailbox) {
      return [];
    }
    const initToken: string = embedPortInit(this.webTag);
    if (initToken.length === 0) {
      return [];
    }
    const item: ScriptItem = {
      script: buildEmbedBootstrapScript(initToken),
      scriptRules: ['*']
    };
    return [item];
  }

  /** The same document: equal URLs once any fragment is dropped, never about:blank. */
  private isSameDocumentUrl(started: string, finished: string): boolean {
    if (!started || !finished) {
      return false;
    }
    const finishedDocument = this.withoutFragment(finished);
    return finishedDocument !== 'about:blank' && this.withoutFragment(started) === finishedDocument;
  }

  private withoutFragment(url: string): string {
    const hash = url.indexOf('#');
    return hash < 0 ? url : url.substring(0, hash);
  }

  private commitDocument(url: string): void {
    if (this.documentCommitted || !this.pageEpoch || !url) {
      return;
    }
    this.documentCommitted = WebViewManager.documentCommit(
      this.webTag,
      this.controller,
      this.pageEpoch,
      url
    );
  }

  private pushWebViewState(titleOverride?: string): void {
    try {
      const gen = WebViewManager.getGenerationForController(this.webTag, this.controller);
      if (!gen) {
        return;
      }
      notifyWebViewState(
        this.webTag,
        gen,
        this.controller.getUrl() ?? '',
        titleOverride ?? (this.controller.getTitle() ?? ''),
        this.controller.accessBackward(),
        this.controller.accessForward()
      );
    } catch (error) {
      hilog.warn(DOMAIN, TAG, `notifyWebViewState failed for ${this.webTag}: ${error}`);
    }
  }

  /** The Web component's inspector id, which a touch recognizer names its target by. */
  private webInspectorId(): string {
    return `lx-web-${this.webTag}`;
  }

  @Builder
  private buildManagedWeb(): void {
    Web({
      src: '',
      controller: this.controller,
      renderMode: this.syncRender ? RenderMode.SYNC_RENDER : RenderMode.ASYNC_RENDER,
      incognitoMode: isEphemeralDataMode(this.webTag)
    })
      .id(this.webInspectorId())
      .darkMode(this.appearanceDark ? WebDarkMode.On : WebDarkMode.Off)
      // Form controls are web-rendered (lx-input is a real <input> in the
      // page); RESIZE_CONTENT lets ArkWeb shrink the page and scroll the
      // focused editable above the IME — engine-owned keyboard avoidance,
      // replacing the removed native-embed input's manual position sync.
      .keyboardAvoidMode(WebKeyboardAvoidMode.RESIZE_CONTENT)
      .javaScriptOnDocumentStart(this.documentStartScripts())
      .multiWindowAccess(this.openNewWindowExternally || supportsWebViewNewWindows(this.webTag))
      .allowWindowOpenMethod(this.openNewWindowExternally || supportsWebViewNewWindows(this.webTag))
      .onWindowNew((event) => {
        const token = WebViewManager.getGenerationForController(this.webTag, this.controller);
        if (!event || !token) {
          return;
        }
        if (!this.openNewWindowExternally && !supportsWebViewNewWindows(this.webTag)) {
          return;
        }
        try {
          event.handler.setWebController(null);
        } catch (error) {
          hilog.warn(DOMAIN, TAG, `onWindowNew: setWebController(null) failed: ${error}`);
        }
        const targetUrl = String(event.targetUrl ?? '').trim();
        if (this.openNewWindowExternally) {
          if (urlCallbackDispatch(targetUrl)) {
            return;
          }
          if (isExternalWindowUrl(targetUrl)) {
            Lingxia.launchWithUrl(targetUrl);
          }
          return;
        }
        if (handleWebViewNewWindowPolicy(this.webTag, token, targetUrl) === 1) {
          WebViewManager.loadUrl(this.webTag, targetUrl);
        }
      })
      .onFullScreenEnter((event) => {
        if (!this.rules().embedMailbox || !event) {
          return;
        }
        const token = WebViewManager.getGenerationForController(this.webTag, this.controller);
        if (!token) {
          return;
        }
        registerFullScreenExitHandler(this.webTag, event.handler);
        onWebFullscreenChanged(this.webTag, token, true);
      })
      .onFullScreenExit(() => {
        if (!this.rules().embedMailbox) {
          return;
        }
        registerFullScreenExitHandler(this.webTag, null);
        const token = WebViewManager.getGenerationForController(this.webTag, this.controller);
        if (token) {
          onWebFullscreenChanged(this.webTag, token, false);
        }
      })
      .onGeolocationShow((event) => {
        if (!event) {
          return;
        }
        const token = WebViewManager.getGenerationForController(this.webTag, this.controller);
        if (!this.rules().embedMailbox || !token || !event.origin
          || !checkWebPermissionPolicy(this.webTag, token, event.origin)) {
          event.geolocation.invoke(event.origin ?? '', false, false);
          return;
        }
        PermissionManager.ensureLocationAccess()
          .then((granted: boolean) => {
            event.geolocation.invoke(event.origin, granted, false);
          })
          .catch(() => {
            event.geolocation.invoke(event.origin, false, false);
          });
      })
      .onPermissionRequest((event) => {
        if (!event || !event.request) {
          return;
        }
        const req = event.request;
        const token = WebViewManager.getGenerationForController(this.webTag, this.controller);
        const origin = req.getOrigin() ?? '';
        if (!this.rules().embedMailbox || !token || !origin
          || !checkWebPermissionPolicy(this.webTag, token, origin)) {
          req.deny();
          return;
        }
        const requested = req.getAccessibleResource() ?? [];
        const grantList: Array<string> = [];
        let needCamera = false;
        let needMic = false;
        for (let i = 0; i < requested.length; i++) {
          const item = requested[i];
          if (item === 'TYPE_VIDEO_CAPTURE') {
            needCamera = true;
            grantList.push(item);
          } else if (item === 'TYPE_AUDIO_CAPTURE') {
            needMic = true;
            grantList.push(item);
          }
        }
        if (grantList.length === 0) {
          req.deny();
          return;
        }
        const checkPromise = needCamera
          ? PermissionManager.ensureCameraAccess(needMic)
          : PermissionManager.ensureMicrophoneAccess();
        checkPromise
          .then((granted: boolean) => {
            if (granted) {
              req.grant(grantList);
            } else {
              req.deny();
            }
          })
          .catch(() => {
            req.deny();
          });
      })
      .onLoadIntercept((event) => {
        if (event && event.data) {
          const request = event.data;
          const url = request.getRequestUrl();
          if (url) {
            let hasUserGesture = false;
            let isMainFrame = true;
            try {
              hasUserGesture = request.isRequestGesture();
            } catch {}
            try {
              isMainFrame = request.isMainFrame();
            } catch {}
            return handleWebViewNavigationPolicy(
              this.webTag,
              WebViewManager.getGenerationForController(this.webTag, this.controller),
              url,
              hasUserGesture,
              isMainFrame
            );
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
          WebViewManager.getGenerationForController(this.webTag, this.controller),
          event.url ?? '',
          event.userAgent ?? '',
          event.contentDisposition ?? '',
          event.mimetype ?? '',
          event.contentLength ?? -1
        );
      })
      .onErrorReceive((event) => {
        if (!event || !event.request || !event.error || !event.request.isMainFrame()) {
          return;
        }
        this.mainFrameFailed = true;
        handleWebViewLoadError(
          this.webTag,
          WebViewManager.getGenerationForController(this.webTag, this.controller),
          this.pageEpoch,
          event.request.getRequestUrl() ?? '',
          event.error.getErrorCode(),
          event.error.getErrorInfo() ?? 'unknown error'
        );
      })
      .onPageBegin((event) => {
        this.documentCommitted = false;
        this.pageBeginUrl = event?.url ?? '';
        this.mainFrameFailed = false;
        this.pageEpoch = WebViewManager.pageBegin(
          this.webTag,
          this.controller,
          event?.url ?? ''
        );
      })
      .onPageVisible(() => {
        // Commit with the URL this begin reported: native matches a commit
        // against it exactly. After loadData ArkWeb can report the begin under
        // the document's own data: URL while getUrl() already returns the
        // history URL, and that mismatch revoked the load, leaving the page
        // without a bridge.
        this.commitDocument(this.pageBeginUrl || (this.controller.getUrl() ?? ''));
      })
      .onTitleReceive((event) => {
        if (event?.title) {
          this.pushWebViewState(event.title);
        }
      })
      .onRefreshAccessedHistory(() => {
        this.pushWebViewState();
      })
      .onPageEnd((event) => {
        // lxapp fallback when ArkWeb never delivers onPageVisible, for the
        // document this begin started only: not after a main-frame error, not
        // about:blank, and not an end that names another URL (a late end for a
        // replaced document). Commit with the begin URL: native matches it
        // exactly, and a fragment-only difference must not revoke the load.
        // BrowserControl keeps requiring visible-commit proof.
        if (!this.documentCommitted
          && !this.rules().provenDocumentBinding
          && !this.mainFrameFailed
          && this.isSameDocumentUrl(this.pageBeginUrl, event?.url ?? '')) {
          this.commitDocument(this.pageBeginUrl);
        }
        this.pushWebViewState();
        WebViewManager.pageEnd(
          this.webTag,
          this.controller,
          this.pageEpoch,
          event?.url ?? ''
        );
      })
      .onRenderExited(() => {
        this.pageEpoch = '';
        this.documentCommitted = false;
        this.pageBeginUrl = '';
        this.mainFrameFailed = false;
        WebViewManager.renderExited(this.webTag, this.controller);
      })
      .onShowFileSelector((event) => {
        if (!event) {
          return false;
        }
        return requestWebFileChooser(
          this.webTag,
          WebViewManager.getGenerationForController(this.webTag, this.controller),
          (selectedPaths: Array<string>) => {
            event.result.handleFileList(selectedPaths);
          },
          event.fileSelector.getAcceptType() ?? [],
          event.fileSelector.getMode(),
          event.fileSelector.isCapture(),
          ''
        );
      })
      .onAlert((event) => {
        if (this.rules().jsDialogs) {
          return false;
        }
        hilog.info(DOMAIN, TAG, `Suppressed JavaScript alert for ${this.webTag}: ${event.url}`);
        event.result.handleConfirm();
        return true;
      })
      .onConfirm((event) => {
        if (this.rules().jsDialogs) {
          return false;
        }
        hilog.info(DOMAIN, TAG, `Suppressed JavaScript confirm for ${this.webTag}: ${event.url}`);
        event.result.handleCancel();
        return true;
      })
      .onPrompt((event) => {
        if (this.rules().jsDialogs) {
          return false;
        }
        hilog.info(DOMAIN, TAG, `Suppressed JavaScript prompt for ${this.webTag}: ${event.url}`);
        event.result.handleCancel();
        return true;
      })
      .onAppear(() => {
        hilog.info(DOMAIN, TAG, `WebView component appeared for webTag: ${this.webTag}`);
        if (!this.controllerAcked) {
          const nativeViewToken = WebViewManager.getGenerationForController(this.webTag, this.controller);
          if (nativeViewToken) {
            notifyWebViewControllerAttached(this.webTag, nativeViewToken);
            this.controllerAcked = true;
          }
        }
      })
      .onControllerAttached(() => {
        if (this.rules().nativeComponentBridge) {
          this.setupEmbedManager();
          hilog.info(DOMAIN, TAG, `Same-level embed manager setup for webTag: ${this.webTag}`);
        }
      })
      .onDisAppear(() => {
        this.cleanupEmbedManager();
        hilog.info(DOMAIN, TAG, `Same-level cleanup for webTag: ${this.webTag}`);
      })
      .verticalScrollBarAccess(this.rules().scrollbars)
      .horizontalScrollBarAccess(this.rules().scrollbars)
      .enableNativeEmbedMode(this.rules().nativeComponentBridge)
      .onNativeEmbedLifecycleChange((embed: NativeEmbedDataInfo) => {
        if (this.rules().nativeComponentBridge && this.embedManager) {
          this.embedManager.setUIContext(this.getUIContext());
          this.embedManager.handleLifecycleChange(embed);
        }
      })
      .onNativeEmbedGestureEvent((touch: NativeEmbedTouchInfo) => {
        if (this.rules().nativeComponentBridge && this.embedManager) {
          this.embedManager.handleGestureEvent(touch);
        }
      })
      .domStorageAccess(this.rules().siteStorage)
      .fileAccess(false)
      .databaseAccess(this.rules().siteStorage)
      .mixedMode(MixedMode.None)
      // Transparent keeps the light-container behavior; a dark lxapp must
      // paint its own canvas or the container bleeds white before first paint.
      .backgroundColor(this.appearanceDark ? '#1C1C1E' : Color.Transparent)
      .visibility(this.renderVisibility)
      .margin(this.renderMargin)
      .width(this.renderWidth)
      .height(this.renderHeight)
      .padding(this.renderPadding)
      .borderRadius(this.renderBorderRadius)
      .clip(this.renderClip)
      .zIndex(this.renderZIndex)
      .hitTestBehavior(this.renderHitTest)
      .onGestureRecognizerJudgeBegin((event: BaseGestureEvent, current: GestureRecognizer,
        recognizers: Array<GestureRecognizer>, touchRecognizers?: Array<TouchRecognizer>): GestureJudgeResult => {
        if (this.yieldsPan !== undefined && current.isBuiltIn()
          && current.getType() === GestureControl.GestureType.PAN_GESTURE) {
          const pan = event as PanGestureEvent;
          if (this.yieldsPan(pan.offsetX, pan.offsetY)) {
            // The page saw the finger go down and move; the screen takes the drag, so the page
            // gets a touchcancel rather than a touchend that would finish its gesture.
            const id = this.webInspectorId();
            const touches: Array<TouchRecognizer> = touchRecognizers ?? [];
            const targets: string[] = [];
            let cancelled = 0;
            for (const touch of touches) {
              const target = touch.getEventTargetInfo().getId();
              targets.push(target);
              if (target === id) {
                touch.cancelTouch();
                cancelled++;
              }
            }
            // Device runs read whether the page's touch recognizer was found and cancelled.
            LxLog.i(TAG, `webview yields pan ${id}: touch recognizers [${targets.join(',')}], cancelled ${cancelled}`);
            return GestureJudgeResult.REJECT;
          }
        }
        return GestureJudgeResult.CONTINUE;
      })
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
      Stack({ alignContent: Alignment.TopStart }) {
        NodeContainer(this.embedManager.getNodeController(componentId)!)
          .width(this.getNodeContainerWidth(componentId))
          .height(this.getNodeContainerHeight(componentId))
          .borderRadius(this.getNodeContainerCornerRadius(componentId))
          .clip(true)
          .position({ x: -this.nodeClipX(componentId), y: -this.nodeClipY(componentId) })
          .hitTestBehavior(this.getNodeContainerHitTest(componentId))
      }
      .width(this.embedManager.getNodeController(componentId)?.getClip()?.width ?? this.getNodeContainerWidth(componentId))
      .height(this.embedManager.getNodeController(componentId)?.getClip()?.height ?? this.getNodeContainerHeight(componentId))
      .clip(true)
      .opacity(this.embedManager.getNodeController(componentId)?.getVideoOpacity() ?? 1)
      .position(this.clippedNodePosition(componentId))
      .zIndex(this.getNodeContainerZIndex(componentId))
      .hitTestBehavior(HitTestMode.Transparent)
    }
  }

  private nodeClipX(componentId: string): number {
    return this.embedManager?.getNodeController(componentId)?.getClip()?.x ?? 0;
  }

  private nodeClipY(componentId: string): number {
    return this.embedManager?.getNodeController(componentId)?.getClip()?.y ?? 0;
  }

  private clippedNodePosition(componentId: string): Position | undefined {
    const controller = this.embedManager?.getNodeController(componentId);
    if (controller?.getClip()) {
      return { x: controller.getX() + this.nodeClipX(componentId), y: controller.getY() + this.nodeClipY(componentId) };
    }
    return this.getNodeContainerPosition(componentId);
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
