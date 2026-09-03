import { ensureComponentId } from "./component.js";
import { registerNativeComponentHandler } from "./nativecomponent.js";
import { findInlineNativeRoot } from "./inline-native/structure.js";

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
  qualities?: LxVideoQuality[];
  playbackRates?: number[];
  className?: string;
  style?: unknown;
  ref?: unknown;
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

/**
 * The DOM leaf for an inline-native video.
 *
 * Rendering, geometry and lifecycle are owned by the nearest LxNativeRoot.
 * A bare lx-video is invalid and deliberately has no legacy overlay path.
 */
export class LxVideoElement extends HTMLElement {
  static get observedAttributes(): string[] {
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
      "playback-rates",
    ];
  }

  private componentId: string | null = null;
  private islandPlaybackIdle = false;
  private unregister?: () => void;
  private handlers: Record<string, EventListenerOrEventListenerObject> = {};
  private rawHandlers: Record<string, EventListenerOrEventListenerObject> = {};
  private bindings: Record<string, string> = {};

  set pageBindings(bindings: Record<string, string>) {
    this.bindings = bindings ?? {};
    if (this.isConnected) this.requestRootCompile();
  }

  get pageBindings(): Record<string, string> {
    return this.bindings;
  }

  get pageFuncBindings(): Record<string, string> {
    return this.bindings;
  }

  get pageFuncBindingsJson(): string {
    return JSON.stringify(this.bindings);
  }

  set src(value: string | null | undefined) {
    if (value == null) {
      this.removeAttribute("src");
    } else {
      this.setAttribute("src", String(value));
    }
  }

  get src(): string | null {
    return this.getAttribute("src");
  }

  set contentRotate(value: unknown) {
    const normalized = this.parseRotateValue(value);
    if (normalized === undefined) {
      this.removeAttribute("content-rotate");
    } else {
      this.setAttribute("content-rotate", String(normalized));
    }
  }

  get contentRotate(): 0 | 90 | 180 | 270 | undefined {
    return this.parseRotateValue(this.getAttribute("content-rotate"));
  }

  connectedCallback(): void {
    for (const property of [
      "pageBindings",
      "contentRotate",
      "src",
      "onplayrequest",
      "onplay",
      "onplaying",
      "onpause",
      "onstop",
      "onended",
      "ontimeupdate",
      "onloadedmetadata",
      "onfullscreenchange",
      "onwaiting",
      "onqualitychange",
      "onratechange",
    ]) {
      this.upgradeProperty(property);
    }

    this.componentId = ensureComponentId(this, "lx-video", this.componentId);
    this.registerEventBridge();

    if (!findInlineNativeRoot(this)) {
      queueMicrotask(() => {
        if (!this.isConnected || findInlineNativeRoot(this)) return;
        this.dispatchEvent(
          new CustomEvent("error", {
            detail: {
              code: "NATIVE_ROOT_INVALID_STRUCTURE",
              message: "LxVideo must be a direct child of LxNativeRoot",
            },
          })
        );
      });
      this.ensurePlaceholderStyle();
      return;
    }

    if (this.hasAttribute("autoplay")) {
      const latchAutoplay = () => {
        if (!this.isConnected || this.islandPlaybackIdle || !this.hasAttribute("autoplay")) return;
        this.setAttribute("data-lx-playing", "true");
      };
      if (typeof requestAnimationFrame === "function") {
        requestAnimationFrame(latchAutoplay);
      } else {
        queueMicrotask(latchAutoplay);
      }
    }
    this.ensurePlaceholderStyle();
  }

  disconnectedCallback(): void {
    this.unregister?.();
    this.unregister = undefined;
    for (const [name, handler] of Object.entries(this.handlers)) {
      this.removeEventListener(name, handler);
    }
    this.handlers = {};
    this.rawHandlers = {};
  }

  attributeChangedCallback(name: string): void {
    if (name === "id" && this.isConnected) {
      const previous = this.componentId;
      this.componentId = ensureComponentId(this, "lx-video", this.componentId);
      if (previous !== this.componentId) this.registerEventBridge();
    }
    if (name === "poster" || name === "object-fit") {
      this.syncPosterPlaceholder();
    }
  }

  private registerEventBridge(): void {
    this.unregister?.();
    if (!this.componentId) return;
    this.unregister = registerNativeComponentHandler(this.componentId, (message) => {
      if (!message.event) return;
      let detail = message.detail || message.payload || {};
      if (
        ["playrequest", "play", "playing", "pause", "stop", "ended", "waiting"].includes(
          message.event
        ) &&
        Object.keys(detail).length === 0
      ) {
        detail = {};
      }
      if (message.event === "playing" || message.event === "play") {
        this.islandPlaybackIdle = false;
        this.setAttribute("data-lx-playing", "true");
      } else if (["pause", "stop", "ended"].includes(message.event)) {
        this.islandPlaybackIdle = true;
        this.removeAttribute("data-lx-playing");
      }
      this.dispatchEvent(
        new CustomEvent(message.event, {
          detail,
          bubbles: true,
          cancelable: false,
        })
      );
    });
  }

  private requestRootCompile(): void {
    const root = findInlineNativeRoot(this) as (HTMLElement & { retry?: () => Promise<void> }) | null;
    void root?.retry?.();
  }

  private upgradeProperty(property: string): void {
    const self = this as unknown as Record<string, unknown>;
    if (!Object.prototype.hasOwnProperty.call(self, property)) return;
    const value = self[property];
    delete self[property];
    self[property] = value;
  }

  private parseRotateValue(value: unknown): 0 | 90 | 180 | 270 | undefined {
    const parsed =
      typeof value === "number"
        ? value
        : typeof value === "string" && /^(0|90|180|270)$/.test(value.trim())
          ? Number(value)
          : Number.NaN;
    return parsed === 0 || parsed === 90 || parsed === 180 || parsed === 270
      ? parsed
      : undefined;
  }

  private parseObjectFitValue(value: unknown): LxObjectFit | undefined {
    if (typeof value !== "string") return undefined;
    const normalized = value.trim().toLowerCase();
    return normalized === "cover" ||
      normalized === "contain" ||
      normalized === "fill" ||
      normalized === "fit"
      ? normalized
      : undefined;
  }

  private isEventListener(value: unknown): value is EventListenerOrEventListenerObject {
    return (
      typeof value === "function" ||
      (!!value &&
        typeof value === "object" &&
        typeof (value as EventListenerObject).handleEvent === "function")
    );
  }

  private setEventHandler(name: string, value: unknown): void {
    const current = this.handlers[name];
    if (current) this.removeEventListener(name, current);
    if (!this.isEventListener(value)) {
      delete this.handlers[name];
      delete this.rawHandlers[name];
      return;
    }
    const listener =
      typeof value === "function" ? ({ handleEvent: value } as EventListenerObject) : value;
    this.handlers[name] = listener;
    this.rawHandlers[name] = value;
    this.addEventListener(name, listener);
  }

  private getEventHandler(name: string): any {
    return this.rawHandlers[name] || null;
  }

  set onplayrequest(value: EventListener) { this.setEventHandler("playrequest", value); }
  get onplayrequest() { return this.getEventHandler("playrequest"); }
  set onplay(value: EventListener) { this.setEventHandler("play", value); }
  get onplay() { return this.getEventHandler("play"); }
  set onplaying(value: EventListener) { this.setEventHandler("playing", value); }
  get onplaying() { return this.getEventHandler("playing"); }
  set onpause(value: EventListener) { this.setEventHandler("pause", value); }
  get onpause() { return this.getEventHandler("pause"); }
  set onstop(value: EventListener) { this.setEventHandler("stop", value); }
  get onstop() { return this.getEventHandler("stop"); }
  set onended(value: EventListener) { this.setEventHandler("ended", value); }
  get onended() { return this.getEventHandler("ended"); }
  set ontimeupdate(value: EventListener) { this.setEventHandler("timeupdate", value); }
  get ontimeupdate() { return this.getEventHandler("timeupdate"); }
  set onloadedmetadata(value: EventListener) { this.setEventHandler("loadedmetadata", value); }
  get onloadedmetadata() { return this.getEventHandler("loadedmetadata"); }
  set onfullscreenchange(value: EventListener) { this.setEventHandler("fullscreenchange", value); }
  get onfullscreenchange() { return this.getEventHandler("fullscreenchange"); }
  set onwaiting(value: EventListener) { this.setEventHandler("waiting", value); }
  get onwaiting() { return this.getEventHandler("waiting"); }
  set onqualitychange(value: EventListener) { this.setEventHandler("qualitychange", value); }
  get onqualitychange() { return this.getEventHandler("qualitychange"); }
  set onratechange(value: EventListener) { this.setEventHandler("ratechange", value); }
  get onratechange() { return this.getEventHandler("ratechange"); }

  private ensurePlaceholderStyle(): void {
    if (!this.style.display) this.style.display = "block";
    if (!this.style.position) this.style.position = "relative";
    if (!this.style.backgroundColor) this.style.backgroundColor = "black";
    if (!this.style.aspectRatio) this.style.aspectRatio = "16 / 9";
    this.syncPosterPlaceholder();
  }

  private syncPosterPlaceholder(): void {
    const poster = this.getAttribute("poster");
    if (!poster) {
      this.style.backgroundImage = "";
      return;
    }
    const objectFit = this.parseObjectFitValue(this.getAttribute("object-fit"));
    const size = objectFit === "cover" ? "cover" : objectFit === "fill" ? "100% 100%" : "contain";
    this.style.backgroundImage = `url("${poster.replace(/"/g, '\\"')}")`;
    this.style.backgroundSize = size;
    this.style.backgroundPosition = "center";
    this.style.backgroundRepeat = "no-repeat";
  }
}

export function registerVideoComponent(): void {
  if (!customElements.get("lx-video")) {
    customElements.define("lx-video", LxVideoElement);
  }
}
