import { sendSameLevelMessage, registerSameLevelHandler } from "./samelevel.js";
import { measureElement } from "./dom.js";
import { ensureComponentId, SameLevelUpdateState } from "./component.js";

// Type definitions
export type LxVideoAttributes = {
  id?: string;
  src?: string;
  poster?: string;
  autoplay?: boolean;
  loop?: boolean;
  muted?: boolean;
  controls?: boolean;
  volume?: string | number;
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
      "volume"
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

  connectedCallback() {
    this.componentId = ensureComponentId(this, "lx-video", this.componentId);
    if (!this.componentId) {
      return;
    }
    this.unregister = registerSameLevelHandler(this.componentId!, (message) => {
      // Handle component events from native
      if (message.event) {
        // Normalize detail based on event type per WeChat Mini Program specs
        let detail = message.detail || message.payload || {};

        // Ensure play/pause/ended have empty detail if not provided
        if (['play', 'pause', 'ended'].includes(message.event)) {
            if (Object.keys(detail).length === 0) detail = {};
        }

                const ev = new CustomEvent(message.event, {
                  detail: detail,
                  bubbles: true,
                  cancelable: false
                });
        // console.log(`[LxVideo] Dispatching event: ${message.event}`, detail);
        this.dispatchEvent(ev);
      }
    });
    this.ensurePlaceholderStyle();
    this.mountOrUpdate();
    this.startTracking();
  }

  disconnectedCallback() {
    this.updateState.reset();

    if (this.componentId) {
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
    return this._handlers[name] || null;
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

  set onerror(cb: EventListener) { this.setEventHandler('error', cb); }
  get onerror() { return this.getEventHandler('error'); }

  set onloadedmetadata(cb: EventListener) { this.setEventHandler('loadedmetadata', cb); }
  get onloadedmetadata() { return this.getEventHandler('loadedmetadata'); }

  set onfullscreenchange(cb: EventListener) { this.setEventHandler('fullscreenchange', cb); }
  get onfullscreenchange() { return this.getEventHandler('fullscreenchange'); }

  // Internal
  private ensurePlaceholderStyle() {
    if (!this.style.display) this.style.display = "block";
    if (!this.style.backgroundColor) this.style.backgroundColor = "black";
    if (!this.style.aspectRatio) this.style.aspectRatio = "16 / 9";
  }

  private collectProps() {
    const volumeAttr = this.getAttribute("volume");
    const volume =
      volumeAttr != null ? parseFloat(volumeAttr) : undefined;
    return {
      src: this.getAttribute("src") || undefined,
      poster: this.getAttribute("poster") || undefined,
      autoplay: this.hasAttribute("autoplay"),
      loop: this.hasAttribute("loop"),
      muted: this.hasAttribute("muted"),
      controls: this.hasAttribute("controls"),
      volume: !Number.isNaN(volume ?? NaN) ? volume : undefined
    };
  }

  private mountOrUpdate() {
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
    // Only include props when they changed or on first mount
    if (decision.propsChanged || !this.mounted) {
      payload.props = props;
    }

    sendSameLevelMessage(payload);
    this.mounted = true;
  }

  private updatePosition() {
    if (!this.mounted || !this.componentId) return;
    const { rect } = measureElement(this);
    if (!rect.width || !rect.height) return;
    const zIndex = parseFloat(this.style.zIndex || "0") || 0;
    const decision = this.updateState.decide(rect, null, zIndex);
    if (!decision.shouldSend) return;
    sendSameLevelMessage({
      action: "component.update",
      id: this.componentId,
      rect,
      zIndex
    });
  }

  private startTracking() {
    window.addEventListener("scroll", this.boundUpdatePosition, { passive: true });
    window.addEventListener("resize", this.boundUpdatePosition);
    this.startSizeObserver();
    this.updatePosition();
  }

  private stopTracking() {
    window.removeEventListener("scroll", this.boundUpdatePosition);
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
}

export function registerVideoComponent() {
  if (!customElements.get("lx-video")) {
    customElements.define("lx-video", LxVideoElement);
  }
}
