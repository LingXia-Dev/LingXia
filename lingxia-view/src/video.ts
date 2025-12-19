import { sendSameLevelMessage, registerSameLevelHandler } from "./samelevel.js";
import { measureElement } from "./dom.js";
import { ensureComponentId, SameLevelUpdateState, iOSSameLevelHelper } from "./component.js";
import { isHarmony, isIOS } from "./platform.js";

const HARMONY_PROPS_PREFIX = "data:application/json,";

// Type definitions
export type LxVideoQuality = { label: string; url?: string };

export type LxVideoAttributes = {
  id?: string;
  src?: string;
  poster?: string;
  autoplay?: boolean;
  loop?: boolean;
  muted?: boolean;
  controls?: boolean;
  volume?: string | number;
  qualities?: LxVideoQuality[];  // First is default
  playbackRates?: number[];      // First is default
  className?: string;
  style?: any;
  ref?: any;
  // Event handlers
  onPlay?: (e: Event) => void;
  onPause?: (e: Event) => void;
  onEnded?: (e: Event) => void;
  onTimeUpdate?: (e: Event) => void;
  onError?: (e: Event) => void;
  onLoadedMetadata?: (e: Event) => void;
  onFullscreenChange?: (e: Event) => void;
  onWaiting?: (e: Event) => void;
  onQualityChange?: (e: Event) => void;
  onPlaybackRateChange?: (e: Event) => void;
};

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
      "autoplay",
      "loop",
      "muted",
      "controls",
      "volume",
      "qualities",
      "playback-rates"
    ];
  }

  private componentId: string | null = null;
  private mounted = false;
  private updateState = new SameLevelUpdateState();
  private unregister?: () => void;
  private resizeObserver?: ResizeObserver;
  private pendingLayoutFrame: number | null = null;
  private boundUpdatePosition = this.updatePosition.bind(this);
  private _handlers: Record<string, EventListenerOrEventListenerObject> = {};
  private harmonyEmbed?: HTMLEmbedElement;
  private lastHarmonyProps?: string;
  private iOSHelper?: iOSSameLevelHelper;

  connectedCallback() {
    this.componentId = ensureComponentId(this, "lx-video", this.componentId);
    if (!this.componentId) {
      return;
    }
    this.unregister = registerSameLevelHandler(this.componentId!, (message) => {
      // Handle component events from native
      if (message.event) {
        // Normalize detail based on event type
        let detail = message.detail || message.payload || {};

        // Ensure play/pause/ended/waiting have empty detail if not provided
        if (['play', 'pause', 'ended', 'waiting'].includes(message.event)) {
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

    // Setup iOS same-level rendering helper (no-op on other platforms)
    this.iOSHelper = new iOSSameLevelHelper(this, this.componentId);
    this.iOSHelper.setup();

    // On iOS, delay mount to allow WKWebView to create WKChildScrollView for the scroll container.
    if (isIOS()) {
      requestAnimationFrame(() => {
        setTimeout(() => {
          if (this.isConnected) {
            this.mountOrUpdate();
            this.startTracking();
          }
        }, 100);
      });
    } else {
      this.mountOrUpdate();
      this.startTracking();
    }
  }

  disconnectedCallback() {
    this.updateState.reset();

    if (this.componentId && !isHarmony()) {
      sendSameLevelMessage({
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

    // Cleanup manually attached property handlers
    Object.keys(this._handlers).forEach(name => {
      this.removeEventListener(name, this._handlers[name]);
    });
    this._handlers = {};
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
    this.mountOrUpdate();
  }

  private setEventHandler(name: string, value: EventListenerOrEventListenerObject | null) {
    const current = this._handlers[name];
    if (current) {
      this.removeEventListener(name, current);
    }
    if (value) {
      this._handlers[name] = value;
      this.addEventListener(name, value);
    } else {
      delete this._handlers[name];
    }
  }

  private getEventHandler(name: string) {
    return (this._handlers[name] || null) as any;
  }

  // Standard Media Events for React Props (e.g. <lx-video onPlay={...} />)
  set onplay(cb: EventListener) { this.setEventHandler('play', cb); }
  get onplay() { return this.getEventHandler('play'); }

  set onpause(cb: EventListener) { this.setEventHandler('pause', cb); }
  get onpause() { return this.getEventHandler('pause'); }

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

  set onplaybackratechange(cb: EventListener) { this.setEventHandler('playbackratechange', cb); }
  get onplaybackratechange() { return this.getEventHandler('playbackratechange'); }

  // Internal
  private ensurePlaceholderStyle() {
    if (!this.style.display) this.style.display = "block";
    if (!this.style.position) this.style.position = "relative"; // Needed for iOS scroll container
    if (!this.style.backgroundColor) this.style.backgroundColor = "black";
    if (!this.style.aspectRatio) this.style.aspectRatio = "16 / 9";
  }

  private collectProps() {
    const volumeAttr = this.getAttribute("volume");
    const volume =
      volumeAttr != null ? parseFloat(volumeAttr) : undefined;

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

    return {
      src: this.getAttribute("src") || undefined,
      poster: this.getAttribute("poster") || undefined,
      autoplay: this.hasAttribute("autoplay"),
      loop: this.hasAttribute("loop"),
      muted: this.hasAttribute("muted"),
      controls: this.hasAttribute("controls"),
      volume: !Number.isNaN(volume ?? NaN) ? volume : undefined,
      qualities,
      playbackRates
    };
  }

  private mountOrUpdate() {
    if (isHarmony()) {
      this.mountOrUpdateHarmony();
      return;
    }
    if (!this.componentId) return;
    const { rect, cornerRadius } = measureElement(this);
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
    const decision = this.updateState.decide(rect, props, zIndex, !this.mounted);
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

    sendSameLevelMessage(payload);
    this.mounted = true;
  }

  private updatePosition() {
    if (isHarmony()) {
      this.mountOrUpdateHarmony();
      return;
    }
    if (!this.mounted || !this.componentId) return;
    const { rect, cornerRadius } = measureElement(this);
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

    if (cornerRadius !== undefined) {
      payload.cornerRadius = cornerRadius;
    }

    sendSameLevelMessage(payload);
  }

  /**
   * Start tracking component size changes.
   *
   * Scroll tracking is handled natively on all platforms:
   * JS only needs to handle resize events and initial mount.
   */
  private startTracking() {
    // Only listen for resize - scroll is handled natively on all platforms
    window.addEventListener("resize", this.boundUpdatePosition);
    this.startSizeObserver();
    this.updatePosition();
  }

  private stopTracking() {
    window.removeEventListener("resize", this.boundUpdatePosition);
    this.stopSizeObserver();
    if (this.pendingLayoutFrame !== null) {
      cancelAnimationFrame(this.pendingLayoutFrame);
      this.pendingLayoutFrame = null;
    }
  }

  private startSizeObserver() {
    if (typeof ResizeObserver === "undefined") {
      // Fallback: rely on rAF retry
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

  private mountOrUpdateHarmony() {
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
    const decision = this.updateState.decide(rect, props, zIndex, !this.mounted);
    if (!decision.shouldSend) return;
    this.ensureHarmonyEmbed(rect, props, cornerRadius);
    this.mounted = true;
  }

  private ensureHarmonyEmbed(
    rect: { width: number; height: number },
    props: Record<string, unknown>,
    cornerRadius?: number
  ) {
    // Create embed element only once - ArkWeb triggers DESTROY+CREATE on attribute changes
    if (!this.harmonyEmbed) {
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

      // Set initial props via src attribute
      const encodedProps = this.encodeHarmonyProps(props);
      embed.setAttribute("src", encodedProps);
      embed.setAttribute("data-lx-props", encodedProps);
      this.lastHarmonyProps = encodedProps;

      this.appendChild(embed);
      this.harmonyEmbed = embed;
      return;
    }

    // Only update props via src if they actually changed
    // DO NOT modify type/id/width/height - it causes ArkWeb to recreate the component
    const encodedProps = this.encodeHarmonyProps(props);
    if (encodedProps !== this.lastHarmonyProps) {
      this.harmonyEmbed.setAttribute("src", encodedProps);
      this.harmonyEmbed.setAttribute("data-lx-props", encodedProps);
      this.lastHarmonyProps = encodedProps;
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
}

export function registerVideoComponent() {
  if (!customElements.get("lx-video")) {
    customElements.define("lx-video", LxVideoElement);
  }
}
