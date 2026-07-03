import {
  sendNativeComponentMessage,
  registerNativeComponentHandler,
  addNativeComponentLayoutInvalidationListener
} from "./nativecomponent.js";
import { measureElement } from "./dom.js";
import { ensureComponentId, NativeComponentUpdateState, iOSNativeComponentHelper } from "./component.js";
import { isAndroid, isHarmony, isIOS, isWindows } from "./platform.js";

const HARMONY_PROPS_PREFIX = "data:application/json,";

// Type definitions
export type LxVideoQuality = { label: string; url?: string };
type LxVideoViewEventHandler = (e: Event) => void;

export type LxVideoAttributes = {
  id?: string;
  src?: string;
  poster?: string;
  objectFit?: "cover" | "contain" | "fill" | "fit";
  contentRotate?: 0 | 90 | 180 | 270;
  autoplay?: boolean;
  loop?: boolean;
  muted?: boolean;
  controls?: boolean;
  progressBar?: boolean;
  live?: boolean;
  volume?: string | number;
  qualities?: LxVideoQuality[];  // First is default
  playbackRates?: number[];      // First is default
  className?: string;
  style?: any;
  ref?: any;
  // Event handlers (framework-native syntax)
  onPlayRequest?: LxVideoViewEventHandler;
  onPlay?: LxVideoViewEventHandler;
  onPlaying?: LxVideoViewEventHandler;
  onPause?: LxVideoViewEventHandler;
  onStop?: LxVideoViewEventHandler;
  onEnded?: LxVideoViewEventHandler;
  onTimeUpdate?: LxVideoViewEventHandler;
  onError?: LxVideoViewEventHandler;
  onLoadedMetadata?: LxVideoViewEventHandler;
  onFullscreenChange?: LxVideoViewEventHandler;
  onWaiting?: LxVideoViewEventHandler;
  onQualityChange?: LxVideoViewEventHandler;
  onRateChange?: LxVideoViewEventHandler;
  // Logic bindings (CLI-generated, maps event name → Logic function name)
  pageBindings?: Record<string, string>;
};

type LxObjectFit = "cover" | "contain" | "fill" | "fit";

declare global {
  namespace JSX {
    interface IntrinsicElements {
      "lx-video": LxVideoAttributes;
    }
  }
}

// Component implementation
export class LxVideoElement extends HTMLElement {
  static get observedAttributes() {
    return [
      "id",
      "src",
      "poster",
      "object-fit",
      "content-rotate",
      "autoplay",
      "loop",
      "muted",
      "controls",
      "progress-bar",
      "live",
      "volume",
      "qualities",
      "playback-rates"
    ];
  }

  private componentId: string | null = null;
  private mounted = false;
  private updateState = new NativeComponentUpdateState();
  private unregister?: () => void;
  private resizeObserver?: ResizeObserver;
  private winSettleFrame: number | null = null;
  private pendingLayoutFrame: number | null = null;
  private boundUpdatePosition = this.updatePosition.bind(this);
  private _handlers: Record<string, EventListenerOrEventListenerObject> = {};
  private harmonyEmbed?: HTMLEmbedElement;
  private lastHarmonyProps?: string;
  private pendingHarmonyProps?: Record<string, unknown>;
  private pendingHarmonyRetryTimer: number | null = null;
  private pendingHarmonyRetryCount: number = 0;
  private iOSBootstrapFrame: number | null = null;
  private iOSBootstrapRemaining: number = 0;
  private iOSHelper?: iOSNativeComponentHelper;
  private removeLayoutInvalidationListener?: () => void;
  private forceHarmonyEmbedRecreate: boolean = false;
  private attrObserver?: MutationObserver;
  private resizeEventListener: EventListenerObject = {
    handleEvent: () => this.boundUpdatePosition(),
  };
  private iOSScrollEventListener: EventListenerObject = {
    handleEvent: () => this.boundUpdatePosition(),
  };
  private scrollSettleFrame: number | null = null;
  // Scroll events do not bubble, so a plain window listener misses inner
  // scroll containers; the capture phase sees them all. Any ancestor
  // scrolling moves the element's document rect, so re-measure (coalesced
  // to one update per frame; decide() dedups unchanged rects).
  private containerScrollListener: EventListenerObject = {
    handleEvent: () => {
      if (this.scrollSettleFrame !== null) return;
      this.scrollSettleFrame = requestAnimationFrame(() => {
        this.scrollSettleFrame = null;
        this.updatePosition();
      });
    },
  };
  private rawHandlers: Record<string, EventListenerOrEventListenerObject> = {};
  private _pageBindings: Record<string, string> = {};

  private parseRotateValue(value: unknown): 0 | 90 | 180 | 270 | undefined {
    const parsed = (() => {
      if (typeof value === "number") {
        return Number.isInteger(value) ? value : NaN;
      }
      if (typeof value === "string") {
        const normalized = value.trim();
        if (!/^(0|90|180|270)$/.test(normalized)) return NaN;
        return Number(normalized);
      }
      return NaN;
    })();
    if (parsed === 0 || parsed === 90 || parsed === 180 || parsed === 270) {
      return parsed;
    }
    return undefined;
  }

  private parseObjectFitValue(value: unknown): LxObjectFit | undefined {
    if (typeof value !== "string") return undefined;
    const normalized = value.trim().toLowerCase();
    if (
      normalized === "cover" ||
      normalized === "contain" ||
      normalized === "fill" ||
      normalized === "fit"
    ) {
      return normalized;
    }
    return undefined;
  }

  set pageBindings(bindings: Record<string, string>) {
    this._pageBindings = bindings ?? {};
    if (this.isConnected) {
      this.mountOrUpdate(true);
    }
  }

  get pageBindings(): Record<string, string> {
    return this._pageBindings;
  }

  set src(value: string | null | undefined) {
    if (value == null) {
      this.removeAttribute("src");
      return;
    }
    this.setAttribute("src", String(value));
  }

  get src(): string | null {
    return this.getAttribute("src");
  }

  private dataAttrToDatasetKey(attr: string): string {
    const raw = attr.slice(5).trim();
    if (!raw) return "";
    const parts = raw.split("-").filter(Boolean);
    if (parts.length === 0) return "";
    return parts
      .map((segment, index) => {
        if (index === 0) return segment.toLowerCase();
        return segment.charAt(0).toUpperCase() + segment.slice(1);
      })
      .join("");
  }

  private collectDataset(): Record<string, string> {
    const dataset: Record<string, string> = {};
    const attrs = this.getAttributeNames();
    for (const attr of attrs) {
      if (!attr.startsWith("data-")) continue;
      const key = this.dataAttrToDatasetKey(attr);
      if (!key) continue;
      const value = this.getAttribute(attr);
      if (value == null) continue;
      dataset[key] = value;
    }
    return dataset;
  }

  private upgradeProperty(propName: string): void {
    const self = this as unknown as Record<string, unknown>;
    if (!Object.prototype.hasOwnProperty.call(self, propName)) return;
    const value = self[propName];
    delete self[propName];
    self[propName] = value;
  }

  set contentRotate(value: unknown) {
    const normalized = this.parseRotateValue(value);
    if (normalized === undefined) {
      this.removeAttribute("content-rotate");
      return;
    }
    this.setAttribute("content-rotate", String(normalized));
  }

  get contentRotate(): 0 | 90 | 180 | 270 | undefined {
    return this.parseRotateValue(this.getAttribute("content-rotate"));
  }

  connectedCallback() {
    // React may set custom-element properties before upgrade; replay through setters.
    this.upgradeProperty("pageBindings");
    this.upgradeProperty("contentRotate");
    this.upgradeProperty("src");
    this.upgradeProperty("onplayrequest");
    this.upgradeProperty("onplay");
    this.upgradeProperty("onplaying");
    this.upgradeProperty("onpause");
    this.upgradeProperty("onstop");
    this.upgradeProperty("onended");
    this.upgradeProperty("ontimeupdate");
    this.upgradeProperty("onloadedmetadata");
    this.upgradeProperty("onfullscreenchange");
    this.upgradeProperty("onwaiting");
    this.upgradeProperty("onqualitychange");
    this.upgradeProperty("onratechange");

    this.componentId = ensureComponentId(this, "lx-video", this.componentId);
    if (!this.componentId) {
      return;
    }
    this.unregister = registerNativeComponentHandler(this.componentId!, (message) => {
      // Handle component events from native
      if (message.event) {
        // Normalize detail based on event type
        let detail = message.detail || message.payload || {};

        // Ensure common state events have empty detail if not provided
        if (['playrequest', 'play', 'playing', 'pause', 'stop', 'ended', 'waiting'].includes(message.event)) {
            if (Object.keys(detail).length === 0) detail = {};
        }

        const ev = new CustomEvent(message.event, {
                  detail: detail,
                  bubbles: true,
                  cancelable: false
                });
        this.dispatchEvent(ev);
      }
    });
    this.ensurePlaceholderStyle();

    // Setup iOS native component rendering helper (no-op on other platforms)
    this.iOSHelper = new iOSNativeComponentHelper(this, this.componentId);
    this.iOSHelper.setup();
    this.startAttrObserver();

    // iOS bootstrap:
    // avoid hardcoded timeout; run an immediate mount plus a few frame-based refreshes.
    if (isIOS()) {
      this.mountOrUpdate(true);
      this.startTracking();
      this.scheduleIOSBootstrapFrames();
    } else {
      this.mountOrUpdate();
      this.startTracking();
      // Windows only: the WebView2 surface is created at an intermediate size
      // and resized to its final content bounds shortly after this element
      // mounts, which reflows the page and moves the element. Unlike the other
      // platforms' webviews, WebView2 doesn't reliably fire the resize /
      // ResizeObserver that would make us re-report, and an early update can be
      // lost in the bridge transport during page load — so the native overlay
      // can stay pinned at the mount-time rect. Re-measure + re-report across the
      // settle window to land the final position. No-op on every other platform.
      if (isWindows()) {
        this.scheduleWindowsMountSettle();
      }
    }
  }

  disconnectedCallback() {
    this.updateState.reset();

    if (this.componentId && !isHarmony()) {
      sendNativeComponentMessage({
        action: "component.unmount",
        id: this.componentId
      });
    }
    this.stopTracking();
    if (this.unregister) {
      this.unregister();
      this.unregister = undefined;
    }
    if (this.harmonyEmbed && this.contains(this.harmonyEmbed)) {
      this.removeChild(this.harmonyEmbed);
    }
    this.harmonyEmbed = undefined;
    this.lastHarmonyProps = undefined;

    // Cleanup iOS helper
    this.iOSHelper?.cleanup();
    this.iOSHelper = undefined;
    this.stopAttrObserver();

    // Cleanup manually attached property handlers
    Object.keys(this._handlers).forEach(name => {
      this.removeEventListener(name, this._handlers[name]);
    });
    this._handlers = {};
    this.rawHandlers = {};
  }

  attributeChangedCallback(name: string) {
    if (name === "id") {
      const prev = this.componentId;
      this.componentId = ensureComponentId(this, "lx-video", this.componentId);
      if (prev && this.componentId !== prev) {
        this.mounted = false;
        this.updateState.reset();
      }
    }
    if (!this.isConnected) return;
    if (name === "poster" || name === "object-fit" || name === "objectfit") {
      this.syncPosterPlaceholder();
    }
    // Keep content-rotate/object-fit updates immediate across all native platforms.
    const forcePropsUpdate = name === "content-rotate" || name === "object-fit" || name === "objectfit";
    if (isHarmony() && forcePropsUpdate) {
      // Bridge callbacks can race on Harmony; force a deterministic embed recreate for visual props.
      this.forceHarmonyEmbedRecreate = true;
    }
    this.mountOrUpdate(forcePropsUpdate);
  }

  private isEventListener(value: unknown): value is EventListenerOrEventListenerObject {
    if (typeof value === "function") return true;
    if (!value || typeof value !== "object") return false;
    return typeof (value as EventListenerObject).handleEvent === "function";
  }

  private setEventHandler(name: string, value: unknown) {
    const current = this._handlers[name];
    if (current) {
      this.removeEventListener(name, current);
    }

    if (this.isEventListener(value)) {
      const listener =
        typeof value === "function"
          ? ({ handleEvent: value } as EventListenerObject)
          : value;
      this._handlers[name] = listener;
      this.rawHandlers[name] = value;
      this.addEventListener(name, listener);
    } else {
      delete this._handlers[name];
      delete this.rawHandlers[name];
    }
  }

  private getEventHandler(name: string) {
    return (this.rawHandlers[name] || null) as any;
  }

  // Standard Media Events for React Props (e.g. <lx-video onPlay={...} />)
  set onplayrequest(cb: EventListener) { this.setEventHandler('playrequest', cb); }
  get onplayrequest() { return this.getEventHandler('playrequest'); }

  set onplay(cb: EventListener) { this.setEventHandler('play', cb); }
  get onplay() { return this.getEventHandler('play'); }

  set onplaying(cb: EventListener) { this.setEventHandler('playing', cb); }
  get onplaying() { return this.getEventHandler('playing'); }

  set onpause(cb: EventListener) { this.setEventHandler('pause', cb); }
  get onpause() { return this.getEventHandler('pause'); }

  set onstop(cb: EventListener) { this.setEventHandler('stop', cb); }
  get onstop() { return this.getEventHandler('stop'); }

  set onended(cb: EventListener) { this.setEventHandler('ended', cb); }
  get onended() { return this.getEventHandler('ended'); }

  set ontimeupdate(cb: EventListener) { this.setEventHandler('timeupdate', cb); }
  get ontimeupdate() { return this.getEventHandler('timeupdate'); }

  // Note: onerror is inherited from HTMLElement with OnErrorEventHandler type.
  // Use addEventListener('error', handler) for error events instead.

  set onloadedmetadata(cb: EventListener) { this.setEventHandler('loadedmetadata', cb); }
  get onloadedmetadata() { return this.getEventHandler('loadedmetadata'); }

  set onfullscreenchange(cb: EventListener) { this.setEventHandler('fullscreenchange', cb); }
  get onfullscreenchange() { return this.getEventHandler('fullscreenchange'); }

  set onwaiting(cb: EventListener) { this.setEventHandler('waiting', cb); }
  get onwaiting() { return this.getEventHandler('waiting'); }

  set onqualitychange(cb: EventListener) { this.setEventHandler('qualitychange', cb); }
  get onqualitychange() { return this.getEventHandler('qualitychange'); }

  set onratechange(cb: EventListener) { this.setEventHandler('ratechange', cb); }
  get onratechange() { return this.getEventHandler('ratechange'); }

  // Internal
  private ensurePlaceholderStyle() {
    if (!this.style.display) this.style.display = "block";
    if (!this.style.position) this.style.position = "relative"; // Needed for iOS scroll container
    if (!this.style.backgroundColor) this.style.backgroundColor = "black";
    if (!this.style.aspectRatio) this.style.aspectRatio = "16 / 9";
    this.syncPosterPlaceholder();
  }

  /// The poster doubles as the element's CSS background: on platforms
  /// where the native layer hides while no media is presented (Windows:
  /// initial state, after stop, on error), the placeholder shows it.
  private syncPosterPlaceholder() {
    const poster = this.getAttribute("poster");
    if (!poster) {
      this.style.backgroundImage = "";
      return;
    }
    const objectFit = this.parseObjectFitValue(
      this.getAttribute("object-fit") ?? this.getAttribute("objectfit")
    );
    const size =
      objectFit === "cover" ? "cover" : objectFit === "fill" ? "100% 100%" : "contain";
    this.style.backgroundImage = `url("${poster.replace(/"/g, '\\"')}")`;
    this.style.backgroundSize = size;
    this.style.backgroundPosition = "center";
    this.style.backgroundRepeat = "no-repeat";
  }

  private collectProps() {
    const volumeAttr = this.getAttribute("volume");
    const volume =
      volumeAttr != null ? parseFloat(volumeAttr) : undefined;
    const rawObjectFit =
      this.getAttribute("object-fit") ??
      this.getAttribute("objectfit");
    const objectFit = this.parseObjectFitValue(rawObjectFit);

    // JSON-encoded arrays (React wrapper sets these via JSON.stringify)
    const qualitiesAttr = this.getAttribute("qualities");
    let qualities: LxVideoQuality[] | undefined;
    if (qualitiesAttr) {
      try {
        const parsed = JSON.parse(qualitiesAttr);
        if (Array.isArray(parsed)) {
          qualities = parsed.filter((q): q is LxVideoQuality =>
            q && typeof q === "object" && typeof (q as { label?: unknown }).label === "string"
          );
        }
      } catch {}
    }

    // Parse playbackRates array from JSON attribute
    const playbackRatesAttr = this.getAttribute("playback-rates");
    let playbackRates: number[] | undefined;
    if (playbackRatesAttr) {
      try {
        const parsed = JSON.parse(playbackRatesAttr);
        if (Array.isArray(parsed)) {
          playbackRates = parsed.filter((n): n is number => typeof n === "number");
        }
      } catch {}
    }

    const isLive = this.hasAttribute("live");
    // When live, default progressBar to false unless explicitly enabled.
    const progressBarAttr = this.getAttribute("progress-bar");
    const progressBar = isLive
      ? (progressBarAttr !== null && progressBarAttr !== "false")
      : (progressBarAttr !== "false");

    const rawContentRotate = this.getAttribute("content-rotate");
    const validContentRotate = this.parseRotateValue(rawContentRotate);
    const pageFuncBindings = this._pageBindings;
    const dataset = this.collectDataset();
    const clearProps: string[] = [];
    if (rawContentRotate === null || validContentRotate === undefined) {
      clearProps.push("contentRotate");
    }
    if (rawObjectFit == null || objectFit === undefined) {
      clearProps.push("objectFit");
    }

    return {
      src: this.getAttribute("src") || undefined,
      poster: this.getAttribute("poster") || undefined,
      autoplay: this.hasAttribute("autoplay"),
      loop: this.hasAttribute("loop"),
      muted: this.hasAttribute("muted"),
      controls: this.hasAttribute("controls"),
      progressBar,
      live: isLive,
      volume: !Number.isNaN(volume ?? NaN) ? volume : undefined,
      objectFit,
      contentRotate: validContentRotate,
      ...(clearProps.length > 0 ? { __clearProps: clearProps } : {}),
      qualities,
      playbackRates,
      dataset,
      datasetJson: JSON.stringify(dataset),
      pageFuncBindings,
      pageFuncBindingsJson: JSON.stringify(pageFuncBindings)
    };
  }

  private shouldRefreshForAttribute(name: string): boolean {
    const normalized = name.trim().toLowerCase();
    return normalized.startsWith("data-");
  }

  private startAttrObserver(): void {
    if (typeof MutationObserver === "undefined" || this.attrObserver) {
      return;
    }
    this.attrObserver = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        const attrName = mutation.attributeName;
        if (!attrName || !this.shouldRefreshForAttribute(attrName)) {
          continue;
        }
        this.mountOrUpdate(true);
        break;
      }
    });
    this.attrObserver.observe(this, { attributes: true });
  }

  private stopAttrObserver(): void {
    if (!this.attrObserver) {
      return;
    }
    this.attrObserver.disconnect();
    this.attrObserver = undefined;
  }

  private measureForNative() {
    const measured = measureElement(this);
    if (!isIOS()) {
      return measured;
    }
    const rect = this.getBoundingClientRect();
    return {
      rect: {
        x: rect.left,
        y: rect.top,
        width: rect.width,
        height: rect.height
      },
      cornerRadius: measured.cornerRadius
    };
  }

  private mountOrUpdate(forceUpdate = false) {
    if (isHarmony()) {
      this.mountOrUpdateHarmony(forceUpdate);
      return;
    }
    if (!this.componentId) return;
    const { rect, cornerRadius } = this.measureForNative();
    const hasSize = rect.width > 0 && rect.height > 0;
    if (!hasSize) {
      // Defer and try again once layout stabilizes; only needed before first mount
      if (!this.mounted) {
        this.scheduleMountRetry();
      }
      return;
    }
    const props = this.collectProps();
    if (cornerRadius !== undefined) {
      (props as any).cornerRadius = cornerRadius;
    }
    const zIndex = parseFloat(this.style.zIndex || "0") || 0;
    const decision = this.updateState.decide(rect, props, zIndex, !this.mounted || forceUpdate);
    if (!decision.shouldSend) return;

    const payload: any = {
      action: this.mounted ? "component.update" : "component.mount",
      id: this.componentId,
      type: "video.native",
      rect,
      zIndex,
      ...(cornerRadius !== undefined ? { cornerRadius } : {})
    };

    // Enhance payload with platform-specific data (iOS scroll container rect)
    this.iOSHelper?.enhancePayload(payload);

    // Only include props when they changed or on first mount
    if (decision.propsChanged || !this.mounted) {
      payload.props = props;
    }

    sendNativeComponentMessage(payload);
    this.mounted = true;
  }

  private updatePosition() {
    if (isHarmony()) {
      this.mountOrUpdateHarmony();
      return;
    }
    if (!this.mounted || !this.componentId) return;
    const { rect, cornerRadius } = this.measureForNative();
    if (!rect.width || !rect.height) return;
    const zIndex = parseFloat(this.style.zIndex || "0") || 0;
    const decision = this.updateState.decide(rect, null, zIndex);
    if (!decision.shouldSend) return;

    const payload: any = {
      action: "component.update",
      id: this.componentId,
      rect,
      zIndex
    };

    // Keep iOS scroll-container metadata on update so native side can still
    // promote fallback overlay into WKChildScrollView when it becomes available.
    this.iOSHelper?.enhancePayload(payload);

    if (cornerRadius !== undefined) {
      payload.cornerRadius = cornerRadius;
    }

    sendNativeComponentMessage(payload);
  }

  /**
   * Start tracking component size changes.
   */
  private startTracking() {
    window.addEventListener("resize", this.resizeEventListener);
    if (isIOS()) {
      window.addEventListener("scroll", this.iOSScrollEventListener, { passive: true });
    } else if (isWindows()) {
      window.addEventListener("scroll", this.containerScrollListener, {
        capture: true,
        passive: true,
      });
    }
    if (isAndroid()) {
      this.removeLayoutInvalidationListener = addNativeComponentLayoutInvalidationListener(this.resizeEventListener);
    }
    this.startSizeObserver();
    this.updatePosition();
  }

  private stopTracking() {
    window.removeEventListener("resize", this.resizeEventListener);
    if (isIOS()) {
      window.removeEventListener("scroll", this.iOSScrollEventListener);
    } else if (isWindows()) {
      window.removeEventListener("scroll", this.containerScrollListener, { capture: true });
    }
    this.removeLayoutInvalidationListener?.();
    this.removeLayoutInvalidationListener = undefined;
    this.stopSizeObserver();
    if (this.scrollSettleFrame !== null) {
      cancelAnimationFrame(this.scrollSettleFrame);
      this.scrollSettleFrame = null;
    }
    if (this.winSettleFrame !== null) {
      cancelAnimationFrame(this.winSettleFrame);
      this.winSettleFrame = null;
    }
    if (this.pendingLayoutFrame !== null) {
      cancelAnimationFrame(this.pendingLayoutFrame);
      this.pendingLayoutFrame = null;
    }
    if (this.pendingHarmonyRetryTimer !== null) {
      clearTimeout(this.pendingHarmonyRetryTimer);
      this.pendingHarmonyRetryTimer = null;
    }
    if (this.iOSBootstrapFrame !== null) {
      cancelAnimationFrame(this.iOSBootstrapFrame);
      this.iOSBootstrapFrame = null;
    }
    this.iOSBootstrapRemaining = 0;
    this.pendingHarmonyProps = undefined;
    this.pendingHarmonyRetryCount = 0;
  }

  private startSizeObserver() {
    if (typeof ResizeObserver === "undefined") {
      // ResizeObserver unavailable: retry mount on next frame.
      this.scheduleMountRetry();
      return;
    }
    if (this.resizeObserver) return;
    this.resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        if (entry.target !== this) continue;
        const { width, height } = entry.contentRect;
        if (!width || !height) continue;
        if (this.mounted) {
          this.updatePosition();
        } else {
          this.mountOrUpdate();
        }
      }
    });
    this.resizeObserver.observe(this);
  }

  private stopSizeObserver() {
    if (this.resizeObserver) {
      this.resizeObserver.disconnect();
      this.resizeObserver = undefined;
    }
  }

  private scheduleMountRetry() {
    if (this.pendingLayoutFrame !== null || this.mounted) return;
    this.pendingLayoutFrame = requestAnimationFrame(() => {
      this.pendingLayoutFrame = null;
      this.mountOrUpdate();
    });
  }

  private scheduleWindowsMountSettle(): void {
    if (typeof requestAnimationFrame === "undefined") return;
    if (this.winSettleFrame !== null) return;
    // Re-measure across the page-load / WebView2-resize settle window. The
    // reflow that moves this element can land well after mount, so keep checking
    // until the rect has been stable for a while; decide() dedups, so updates
    // only fire when the rect changes.
    let lastKey = "";
    let stable = 0;
    let elapsed = 0;
    const tick = () => {
      this.winSettleFrame = null;
      if (!this.isConnected) return;
      // Force the re-send (don't dedup): an early update can be lost in the
      // bridge transport during page load, and once the element has settled a
      // dedup'd re-measure sees "no change" and never re-delivers it. Forcing
      // each frame re-delivers the current rect until it lands.
      this.mountOrUpdate(true);
      const r = this.getBoundingClientRect();
      const key = `${Math.round(r.top)}:${Math.round(r.width)}`;
      if (key === lastKey) {
        stable += 1;
      } else {
        stable = 0;
        lastKey = key;
      }
      elapsed += 1;
      // Keep re-sending until the layout has held steady for a while (so a late
      // settle is still caught), capped so it never runs unbounded.
      if (stable < 12 && elapsed < 180) {
        this.winSettleFrame = requestAnimationFrame(tick);
      }
    };
    this.winSettleFrame = requestAnimationFrame(tick);
  }

  private scheduleIOSBootstrapFrames(): void {
    if (!isIOS()) return;
    // Keep a short frame-based bootstrap window so late-created WKChildScrollView
    // can still receive mount/update without relying on hardcoded millisecond delay.
    this.iOSBootstrapRemaining = 8;
    const tick = () => {
      if (!this.isConnected) {
        this.iOSBootstrapFrame = null;
        this.iOSBootstrapRemaining = 0;
        return;
      }
      this.mountOrUpdate(true);
      this.iOSBootstrapRemaining -= 1;
      if (this.iOSBootstrapRemaining <= 0) {
        this.iOSBootstrapFrame = null;
        return;
      }
      this.iOSBootstrapFrame = requestAnimationFrame(tick);
    };
    this.iOSBootstrapFrame = requestAnimationFrame(tick);
  }

  private mountOrUpdateHarmony(forceUpdate = false) {
    if (!this.componentId) return;
    const { rect, cornerRadius } = measureElement(this);
    const hasSize = rect.width > 0 && rect.height > 0;
    if (!hasSize) {
      if (!this.mounted) {
        this.scheduleMountRetry();
      }
      return;
    }
    const props = this.collectProps();
    if (cornerRadius !== undefined) {
      (props as any).cornerRadius = cornerRadius;
    }
    const zIndex = parseFloat(this.style.zIndex || "0") || 0;
    const decision = this.updateState.decide(rect, props, zIndex, !this.mounted || forceUpdate);
    if (!decision.shouldSend) return;
    this.ensureHarmonyEmbed(
      rect,
      props,
      cornerRadius,
      this.forceHarmonyEmbedRecreate
    );
    this.forceHarmonyEmbedRecreate = false;
    this.mounted = true;
  }

  private ensureHarmonyEmbed(
    rect: { width: number; height: number },
    props: Record<string, unknown>,
    cornerRadius?: number,
    forceRecreate = false
  ) {
    const createEmbed = (): HTMLEmbedElement => {
      const embed = document.createElement("embed");
      embed.setAttribute("type", "native/video");
      embed.setAttribute("width", `${rect.width}`);
      embed.setAttribute("height", `${rect.height}`);
      embed.setAttribute("id", this.componentId!);
      embed.setAttribute("data-lx-component-id", this.componentId!);
      embed.style.display = "block";
      embed.style.width = "100%";
      embed.style.height = "100%";
      embed.style.border = "none";
      if (cornerRadius !== undefined) {
        embed.style.borderRadius = `${cornerRadius}px`;
        embed.style.overflow = "hidden";
      }
      const encodedProps = this.encodeHarmonyProps(props);
      embed.setAttribute("src", encodedProps);
      this.lastHarmonyProps = encodedProps;
      return embed;
    };

    if (forceRecreate && this.harmonyEmbed) {
      const oldEmbed = this.harmonyEmbed;
      this.harmonyEmbed = undefined;
      if (this.contains(oldEmbed)) {
        this.removeChild(oldEmbed);
      }
      this.lastHarmonyProps = undefined;
      const recreated = createEmbed();
      this.appendChild(recreated);
      this.harmonyEmbed = recreated;
      this.pushHarmonyPropsUpdate(props);
      this.pendingHarmonyRetryCount = 0;
      return;
    }

    // Create embed element only once - ArkWeb triggers DESTROY+CREATE on attribute changes
    if (!this.harmonyEmbed) {
      const embed = createEmbed();
      this.pushHarmonyPropsUpdate(props);

      this.appendChild(embed);
      this.harmonyEmbed = embed;
      return;
    }

    // Only update props when they changed.
    const encodedProps = this.encodeHarmonyProps(props);
    if (encodedProps !== this.lastHarmonyProps) {
      const pushed = this.pushHarmonyPropsUpdate(props);
      if (pushed) {
        this.lastHarmonyProps = encodedProps;
        this.pendingHarmonyProps = undefined;
        this.pendingHarmonyRetryCount = 0;
      } else {
        this.scheduleHarmonyPropsRetry(props);
      }
    }

    // Ensure embed element matches latest corner radius for clipping
    if (cornerRadius !== undefined && this.harmonyEmbed.style.borderRadius !== `${cornerRadius}px`) {
      this.harmonyEmbed.style.borderRadius = `${cornerRadius}px`;
      this.harmonyEmbed.style.overflow = "hidden";
    }
  }

  private encodeHarmonyProps(props: Record<string, unknown>): string {
    try {
      const payload = { componentId: this.componentId, ...props };
      return `${HARMONY_PROPS_PREFIX}${encodeURIComponent(
        JSON.stringify(payload)
      )}`;
    } catch (_e) {
      return `${HARMONY_PROPS_PREFIX}%7B%7D`;
    }
  }

  private pushHarmonyPropsUpdate(props: Record<string, unknown>): boolean {
    if (!this.componentId || !isHarmony()) return false;
    try {
      const proxy = (window as unknown as Record<string, unknown>).LingXiaProxy as
        | { nativeComponentUpdate?: ((componentId: string, payload: string) => void) | ((payload: string) => void) }
        | undefined;
      const updateFn = proxy?.nativeComponentUpdate;
      if (typeof updateFn !== "function") {
        return false;
      }
      const payload = JSON.stringify({ componentId: this.componentId, ...props });
      try {
        (updateFn as (payload: string) => void).call(proxy, payload);
        return true;
      } catch {
        try {
          (updateFn as (componentId: string, payload: string) => void).call(proxy, this.componentId, payload);
          return true;
        } catch {
          return false;
        }
      }
    } catch (error) {
      return false;
    }
  }

  private scheduleHarmonyPropsRetry(props: Record<string, unknown>): void {
    this.pendingHarmonyProps = { ...props };
    if (this.pendingHarmonyRetryTimer !== null) {
      return;
    }
    this.pendingHarmonyRetryTimer = window.setTimeout(() => {
      this.pendingHarmonyRetryTimer = null;
      if (!this.isConnected || !this.pendingHarmonyProps) {
        return;
      }
      const pendingProps = this.pendingHarmonyProps;
      const pushed = this.pushHarmonyPropsUpdate(pendingProps);
      if (pushed) {
        this.lastHarmonyProps = this.encodeHarmonyProps(pendingProps);
        this.pendingHarmonyProps = undefined;
        this.pendingHarmonyRetryCount = 0;
      } else {
        this.pendingHarmonyRetryCount += 1;
        if (this.pendingHarmonyRetryCount < 20) {
          this.scheduleHarmonyPropsRetry(pendingProps);
        }
      }
    }, 100);
  }

}

export function registerVideoComponent() {
  if (!customElements.get("lx-video")) {
    customElements.define("lx-video", LxVideoElement);
  }
}
