import {
  sendNativeComponentMessage,
  registerNativeComponentHandler,
  addNativeComponentLayoutInvalidationListener,
} from "./nativecomponent.js";
import { ensureComponentId, NativeComponentUpdateState, iOSNativeComponentHelper } from "./component.js";
import { measureElement } from "./dom.js";
import { isAndroid, isHarmony, isIOS } from "./platform.js";

type LxMediaSwiperEventHandler = (e: Event) => void;

export type LxMediaSwiperItem =
  | {
      id?: string;
      type: "image";
      src: string;
    }
  | {
      id?: string;
      type: "video";
      src: string;
      poster?: string;
      controls?: boolean;
      muted?: boolean;
    };

export type LxMediaSwiperChangeSource = "touch" | "autoplay" | "api" | "video";
export type LxMediaSwiperErrorCode =
  | "not_found"
  | "network"
  | "decode"
  | "unsupported_format"
  | "permission_denied"
  | "unknown";

export type LxMediaSwiperEventDetail = {
  index: number;
  item: LxMediaSwiperItem;
};

export type LxMediaSwiperChangeEventDetail = {
  index: number;
  previousIndex: number;
  item: LxMediaSwiperItem;
  source: LxMediaSwiperChangeSource;
};

export type LxMediaSwiperTransitionEndEventDetail = LxMediaSwiperChangeEventDetail;

export type LxMediaSwiperEndReachedEventDetail = {
  index: number;
  item: LxMediaSwiperItem;
  source: LxMediaSwiperChangeSource;
};

export type LxMediaSwiperErrorEventDetail = {
  index: number;
  item?: LxMediaSwiperItem;
  code: LxMediaSwiperErrorCode;
  message: string;
};

export type LxMediaSwiperChangeEvent = CustomEvent<LxMediaSwiperChangeEventDetail>;
export type LxMediaSwiperTransitionEndEvent = CustomEvent<LxMediaSwiperTransitionEndEventDetail>;
export type LxMediaSwiperItemEvent = CustomEvent<LxMediaSwiperEventDetail>;
export type LxMediaSwiperEndReachedEvent = CustomEvent<LxMediaSwiperEndReachedEventDetail>;
export type LxMediaSwiperErrorEvent = CustomEvent<LxMediaSwiperErrorEventDetail>;

/**
 * Peek leaves part of the previous and next pages visible alongside the current
 * page so users get a hint that more content exists. A single `number` applies
 * symmetric peek on both edges; an object lets the two sides differ.
 *
 * For `direction="vertical"`, `previous` peeks the top edge and `next` peeks
 * the bottom edge.
 *
 * Values are CSS pixels (DIPs). `0` (default) disables peek and behaves
 * identically to v1: each page fills the swiper bounds.
 */
export type LxMediaSwiperPeek = number | { previous?: number; next?: number };

export type LxMediaSwiperAttributes = {
  id?: string;
  items?: LxMediaSwiperItem[];
  index?: number;
  initialIndex?: number;
  loop?: boolean;
  autoplay?: boolean;
  interval?: number;
  animation?: "slide" | "none";
  animationDuration?: number;
  direction?: "horizontal" | "vertical";
  contentRotate?: 0 | 90 | 180 | 270;
  objectFit?: "cover" | "contain" | "fill" | "fit";
  controls?: boolean;
  muted?: boolean;
  dots?: boolean | { color?: string; activeColor?: string };
  swipeEnabled?: boolean;
  peek?: LxMediaSwiperPeek;
  className?: string;
  style?: any;
  ref?: any;
  onChange?: LxMediaSwiperEventHandler;
  onTransitionEnd?: LxMediaSwiperEventHandler;
  onEndReached?: LxMediaSwiperEventHandler;
  onTap?: LxMediaSwiperEventHandler;
  onVideoEnded?: LxMediaSwiperEventHandler;
  onError?: LxMediaSwiperEventHandler;
  pageBindings?: Record<string, string>;
};

type ObjectFit = "cover" | "contain" | "fill" | "fit";
type Animation = "slide" | "none";
type Direction = "horizontal" | "vertical";

const EVENT_MAP = {
  onchange: "change",
  ontransitionend: "transitionend",
  onendreached: "endreached",
  ontap: "tap",
  onvideoended: "videoended",
} as const;

declare global {
  namespace JSX {
    interface IntrinsicElements {
      "lx-media-swiper": LxMediaSwiperAttributes;
    }
  }
}

export class LxMediaSwiperElement extends HTMLElement {
  static get observedAttributes() {
    return [
      "id",
      "items",
      "index",
      "initial-index",
      "loop",
      "autoplay",
      "interval",
      "animation",
      "animation-duration",
      "direction",
      "content-rotate",
      "object-fit",
      "controls",
      "muted",
      "dots",
      "swipe-enabled",
      "peek",
    ];
  }

  private componentId: string | null = null;
  private mounted = false;
  private updateState = new NativeComponentUpdateState();
  private unregister?: () => void;
  private resizeObserver?: ResizeObserver;
  private pendingLayoutFrame: number | null = null;
  private boundUpdatePosition = this.updatePosition.bind(this);
  private removeLayoutInvalidationListener?: () => void;
  private iOSHelper?: iOSNativeComponentHelper;
  private rawHandlers: Record<string, EventListenerOrEventListenerObject> = {};
  private handlers: Record<string, EventListenerOrEventListenerObject> = {};
  private _pageBindings: Record<string, string> = {};
  // Monotonic command counter used as the Harmony bridge fallback. Harmony's
  // Web.onNativeEmbedDataInfo channel only delivers attribute-derived props,
  // so imperative API calls have to piggyback into a prop the Builder polls.
  private harmonyCommandSeq = 0;
  private harmonyPendingCommand: { name: string; params?: Record<string, unknown> } | null = null;
  private pendingHarmonyProps?: Record<string, unknown>;
  private pendingHarmonyRetryTimer: number | null = null;
  private pendingHarmonyRetryCount = 0;
  // Harmony's ArkWeb only fires `onNativeEmbedDataInfo` for actual <embed> elements
  // with `type="native/..."`. Other platforms rely on the bridge message channel
  // (component.mount/update/unmount), but Harmony has no such channel, so props
  // are encoded into the child <embed> src.
  private harmonyEmbed?: HTMLEmbedElement;
  private lastHarmonyProps?: string;

  set items(value: LxMediaSwiperItem[] | string | null | undefined) {
    if (typeof value === "string") {
      const trimmed = value.trim();
      if (trimmed.length === 0) {
        this.removeAttribute("items");
        return;
      }
      this.setAttribute("items", trimmed);
      return;
    }
    if (!Array.isArray(value) || value.length === 0) {
      this.removeAttribute("items");
      return;
    }
    this.setAttribute("items", JSON.stringify(value));
  }

  get items(): LxMediaSwiperItem[] {
    return this.parseItems();
  }

  set index(value: number | null | undefined) {
    if (value == null || !Number.isFinite(Number(value))) {
      this.removeAttribute("index");
      return;
    }
    this.setAttribute("index", String(Math.trunc(Number(value))));
  }

  get index(): number | undefined {
    return this.parseIntegerAttr("index");
  }

  set initialIndex(value: number | null | undefined) {
    if (value == null || !Number.isFinite(Number(value))) {
      this.removeAttribute("initial-index");
      return;
    }
    this.setAttribute("initial-index", String(Math.trunc(Number(value))));
  }

  get initialIndex(): number | undefined {
    return this.parseIntegerAttr("initial-index");
  }

  set contentRotate(value: unknown) {
    const parsed = this.parseRotate(value);
    if (parsed === undefined) {
      this.removeAttribute("content-rotate");
      return;
    }
    this.setAttribute("content-rotate", String(parsed));
  }

  get contentRotate(): 0 | 90 | 180 | 270 | undefined {
    return this.parseRotate(this.getAttribute("content-rotate"));
  }

  set pageBindings(bindings: Record<string, string>) {
    this.setPageBindings(bindings);
  }

  get pageBindings(): Record<string, string> {
    return this._pageBindings;
  }

  set pageFuncBindings(bindings: Record<string, string>) {
    this.setPageBindings(bindings);
  }

  setPageBindings(bindings: Record<string, string>) {
    this._pageBindings = bindings ?? {};
    if (this.isConnected) this.mountOrUpdate(true);
  }

  next(): void {
    this.sendCommand("next");
  }

  previous(): void {
    this.sendCommand("previous");
  }

  goToIndex(index: number): void {
    if (!Number.isInteger(index)) return;
    this.sendCommand("goToIndex", { index });
  }

  connectedCallback() {
    this.upgradeProperty("items");
    this.upgradeProperty("index");
    this.upgradeProperty("initialIndex");
    this.upgradeProperty("contentRotate");
    this.upgradeProperty("pageBindings");
    this.upgradeProperty("pageFuncBindings");
    for (const prop of Object.keys(EVENT_MAP)) this.upgradeProperty(prop);

    this.componentId = ensureComponentId(this, "lx-media-swiper", this.componentId);
    if (!this.componentId) return;

    this.unregister = registerNativeComponentHandler(this.componentId, (message) => {
      if (!message.event) return;
      const detail = message.detail || message.payload || {};
      const ev = new CustomEvent(message.event, {
        detail,
        bubbles: true,
        cancelable: false,
      });
      this.dispatchEvent(ev);
    });

    this.ensurePlaceholderStyle();
    this.iOSHelper = new iOSNativeComponentHelper(this, this.componentId);
    this.iOSHelper.setup();

    this.mountOrUpdate();
    this.startTracking();
  }

  disconnectedCallback() {
    this.updateState.reset();
    if (this.componentId && !isHarmony()) {
      sendNativeComponentMessage({
        action: "component.unmount",
        id: this.componentId,
      });
    }
    if (this.harmonyEmbed && this.contains(this.harmonyEmbed)) {
      this.removeChild(this.harmonyEmbed);
    }
    this.harmonyEmbed = undefined;
    this.lastHarmonyProps = undefined;
    this.harmonyPendingCommand = null;
    this.pendingHarmonyProps = undefined;
    this.pendingHarmonyRetryCount = 0;
    if (this.pendingHarmonyRetryTimer !== null) {
      window.clearTimeout(this.pendingHarmonyRetryTimer);
      this.pendingHarmonyRetryTimer = null;
    }
    this.stopTracking();
    this.unregister?.();
    this.unregister = undefined;
    this.iOSHelper?.cleanup();
    this.iOSHelper = undefined;
    Object.keys(this.handlers).forEach((name) => this.removeEventListener(name, this.handlers[name]));
    this.handlers = {};
    this.rawHandlers = {};
  }

  attributeChangedCallback(name: string) {
    if (name === "id") {
      const prev = this.componentId;
      this.componentId = ensureComponentId(this, "lx-media-swiper", this.componentId);
      if (prev && this.componentId !== prev) {
        this.mounted = false;
        this.updateState.reset();
      }
    }
    if (!this.isConnected) return;
    this.mountOrUpdate(name === "content-rotate" || name === "object-fit");
  }

  set onchange(cb: EventListener) { this.setEventHandler("change", cb); }
  get onchange() { return this.getEventHandler("change"); }

  set ontransitionend(cb: EventListener) { this.setEventHandler("transitionend", cb); }
  get ontransitionend() { return this.getEventHandler("transitionend"); }

  set onendreached(cb: EventListener) { this.setEventHandler("endreached", cb); }
  get onendreached() { return this.getEventHandler("endreached"); }

  set ontap(cb: EventListener) { this.setEventHandler("tap", cb); }
  get ontap() { return this.getEventHandler("tap"); }

  set onvideoended(cb: EventListener) { this.setEventHandler("videoended", cb); }
  get onvideoended() { return this.getEventHandler("videoended"); }

  private sendCommand(name: string, params?: Record<string, unknown>) {
    if (!this.componentId) return;
    if (isHarmony()) {
      // Harmony has no bridge channel for `component.command`; route the call through
      // the props pipeline so the Builder's onParamsChange can dispatch it.
      this.harmonyCommandSeq += 1;
      this.harmonyPendingCommand = { name, params };
      this.mountOrUpdate(true);
      return;
    }
    sendNativeComponentMessage({
      action: "component.command",
      id: this.componentId,
      name,
      ...(params ? { params } : {}),
    });
  }

  private collectProps(): Record<string, unknown> {
    const objectFit = this.parseObjectFit(this.getAttribute("object-fit"));
    const dots = this.parseDots();
    const dataset = this.collectDataset();
    // Always emit a normalised peek object so removing the attribute (or setting it
    // to 0) propagates as an explicit reset. JSON.stringify would drop `undefined`,
    // which makes native think peek hasn't changed and keep the old padding.
    const peek = this.parsePeek() ?? { previous: 0, next: 0 };
    const props: Record<string, unknown> = {
      items: this.parseItems(),
      index: this.parseIntegerAttr("index") ?? null,
      initialIndex: this.parseIntegerAttr("initial-index") ?? 0,
      loop: this.hasAttribute("loop"),
      autoplay: this.hasAttribute("autoplay"),
      interval: this.parseIntegerAttr("interval") ?? 5000,
      animation: this.parseAnimation(this.getAttribute("animation")),
      animationDuration: this.parseIntegerAttr("animation-duration") ?? 300,
      direction: this.parseDirection(this.getAttribute("direction")),
      contentRotate: this.parseRotate(this.getAttribute("content-rotate")) ?? 0,
      objectFit: objectFit ?? "cover",
      controls: this.hasAttribute("controls"),
      muted: this.hasAttribute("muted") ? this.getAttribute("muted") !== "false" : true,
      dots,
      swipeEnabled: this.getAttribute("swipe-enabled") !== "false",
      peek,
      pageFuncBindings: this._pageBindings,
      pageFuncBindingsJson: JSON.stringify(this._pageBindings),
      dataset,
      datasetJson: JSON.stringify(dataset),
    };
    return props;
  }

  private mountOrUpdate(forceUpdate = false) {
    if (isHarmony()) {
      this.mountOrUpdateHarmony(forceUpdate);
      return;
    }
    if (!this.componentId) return;
    const measured = measureElement(this);
    const rect = measured.rect;
    if (!rect.width || !rect.height) {
      if (!this.mounted) this.scheduleMountRetry();
      return;
    }
    const props = this.collectProps();
    const zIndex = parseFloat(this.style.zIndex || "0") || 0;
    const decision = this.updateState.decide(rect, props, zIndex, !this.mounted || forceUpdate);
    if (!decision.shouldSend) return;

    const payload: any = {
      action: this.mounted ? "component.update" : "component.mount",
      id: this.componentId,
      type: "media-swiper.native",
      rect,
      zIndex,
      ...(measured.cornerRadius !== undefined ? { cornerRadius: measured.cornerRadius } : {}),
    };
    this.iOSHelper?.enhancePayload(payload);
    if (decision.propsChanged || !this.mounted) payload.props = props;
    sendNativeComponentMessage(payload);
    this.mounted = true;
  }

  private mountOrUpdateHarmony(forceUpdate = false) {
    if (!this.componentId) return;
    const measured = measureElement(this);
    const rect = measured.rect;
    if (!rect.width || !rect.height) {
      if (!this.mounted) this.scheduleMountRetry();
      return;
    }
    const props = this.collectProps();
    if (measured.cornerRadius !== undefined) {
      (props as any).cornerRadius = measured.cornerRadius;
    }
    const zIndex = parseFloat(this.style.zIndex || "0") || 0;
    const decision = this.updateState.decide(rect, props, zIndex, !this.mounted || forceUpdate);
    if (!decision.shouldSend) return;
    this.ensureHarmonyEmbed(rect, props, measured.cornerRadius);
    this.flushHarmonyCommandIfPending(props);
    this.mounted = true;
  }

  private ensureHarmonyEmbed(
    rect: { width: number; height: number },
    props: Record<string, unknown>,
    cornerRadius?: number
  ) {
    const createEmbed = (): HTMLEmbedElement => {
      const embed = document.createElement("embed");
      embed.setAttribute("type", "native/media-swiper");
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

    if (!this.harmonyEmbed) {
      const embed = createEmbed();
      this.appendChild(embed);
      this.harmonyEmbed = embed;
      this.pushHarmonyPropsUpdate(props);
      return;
    }

    const encodedProps = this.encodeHarmonyProps(props);
    if (encodedProps !== this.lastHarmonyProps) {
      this.harmonyEmbed.setAttribute("src", encodedProps);
      const pushed = this.pushHarmonyPropsUpdate(props);
      if (pushed) {
        this.lastHarmonyProps = encodedProps;
        this.pendingHarmonyProps = undefined;
        this.pendingHarmonyRetryCount = 0;
      } else {
        this.scheduleHarmonyPropsRetry(props);
      }
    }

    if (cornerRadius !== undefined && this.harmonyEmbed.style.borderRadius !== `${cornerRadius}px`) {
      this.harmonyEmbed.style.borderRadius = `${cornerRadius}px`;
      this.harmonyEmbed.style.overflow = "hidden";
    }
  }

  private encodeHarmonyProps(props: Record<string, unknown>): string {
    try {
      const payload = { componentId: this.componentId, ...props };
      return `data:application/json,${encodeURIComponent(JSON.stringify(payload))}`;
    } catch {
      return `data:application/json,%7B%7D`;
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
    } catch {
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

  private flushHarmonyCommandIfPending(baseProps: Record<string, unknown>): void {
    if (!this.harmonyPendingCommand) return;
    const command = this.harmonyPendingCommand;
    const commandProps: Record<string, unknown> = {
      ...baseProps,
      __cmdSeq: this.harmonyCommandSeq,
      __cmdName: command.name,
    };
    if (command.params) {
      commandProps.__cmdParams = command.params;
    }
    if (this.harmonyEmbed) {
      const encodedProps = this.encodeHarmonyProps(commandProps);
      this.harmonyEmbed.setAttribute("src", encodedProps);
      this.lastHarmonyProps = encodedProps;
      if (!this.pushHarmonyPropsUpdate(commandProps)) {
        this.scheduleHarmonyPropsRetry(commandProps);
      }
    }
    this.harmonyPendingCommand = null;
  }

  private updatePosition() {
    if (isHarmony()) {
      // Harmony tracks size via the embed element + ArkWeb's onNativeEmbedDataInfo;
      // any rect/prop refresh has to go through mountOrUpdateHarmony so the embed
      // stays consistent.
      this.mountOrUpdateHarmony();
      return;
    }
    if (!this.mounted || !this.componentId) return;
    const measured = measureElement(this);
    const rect = measured.rect;
    if (!rect.width || !rect.height) return;
    const zIndex = parseFloat(this.style.zIndex || "0") || 0;
    const decision = this.updateState.decide(rect, null, zIndex);
    if (!decision.shouldSend) return;
    const payload: any = {
      action: "component.update",
      id: this.componentId,
      rect,
      zIndex,
      ...(measured.cornerRadius !== undefined ? { cornerRadius: measured.cornerRadius } : {}),
    };
    this.iOSHelper?.enhancePayload(payload);
    sendNativeComponentMessage(payload);
  }

  private startTracking() {
    window.addEventListener("resize", this.boundUpdatePosition);
    if (isAndroid()) {
      this.removeLayoutInvalidationListener = addNativeComponentLayoutInvalidationListener(this.boundUpdatePosition);
    }
    this.startSizeObserver();
    this.updatePosition();
  }

  private stopTracking() {
    window.removeEventListener("resize", this.boundUpdatePosition);
    this.removeLayoutInvalidationListener?.();
    this.removeLayoutInvalidationListener = undefined;
    this.stopSizeObserver();
    if (this.pendingLayoutFrame !== null) {
      cancelAnimationFrame(this.pendingLayoutFrame);
      this.pendingLayoutFrame = null;
    }
  }

  private startSizeObserver() {
    if (typeof ResizeObserver === "undefined" || this.resizeObserver) return;
    this.resizeObserver = new ResizeObserver(() => {
      if (this.pendingLayoutFrame !== null) cancelAnimationFrame(this.pendingLayoutFrame);
      this.pendingLayoutFrame = requestAnimationFrame(() => {
        this.pendingLayoutFrame = null;
        this.mountOrUpdate();
      });
    });
    this.resizeObserver.observe(this);
  }

  private stopSizeObserver() {
    this.resizeObserver?.disconnect();
    this.resizeObserver = undefined;
  }

  private scheduleMountRetry() {
    if (this.pendingLayoutFrame !== null) return;
    this.pendingLayoutFrame = requestAnimationFrame(() => {
      this.pendingLayoutFrame = null;
      if (this.isConnected) this.mountOrUpdate();
    });
  }

  private ensurePlaceholderStyle() {
    if (!this.style.display) this.style.display = "block";
    if (!this.style.position) this.style.position = "relative";
    if (!this.style.backgroundColor) this.style.backgroundColor = "black";
    if (!this.style.aspectRatio) this.style.aspectRatio = "16 / 9";
  }

  private upgradeProperty(propName: string): void {
    const self = this as unknown as Record<string, unknown>;
    if (!Object.prototype.hasOwnProperty.call(self, propName)) return;
    const value = self[propName];
    delete self[propName];
    self[propName] = value;
  }

  private isEventListener(value: unknown): value is EventListenerOrEventListenerObject {
    if (typeof value === "function") return true;
    if (!value || typeof value !== "object") return false;
    return typeof (value as EventListenerObject).handleEvent === "function";
  }

  private setEventHandler(name: string, value: unknown) {
    const current = this.handlers[name];
    if (current) this.removeEventListener(name, current);
    if (this.isEventListener(value)) {
      const listener = typeof value === "function" ? ({ handleEvent: value } as EventListenerObject) : value;
      this.handlers[name] = listener;
      this.rawHandlers[name] = value;
      this.addEventListener(name, listener);
    } else {
      delete this.handlers[name];
      delete this.rawHandlers[name];
    }
  }

  private getEventHandler(name: string) {
    return (this.rawHandlers[name] || null) as any;
  }

  private parseItems(): LxMediaSwiperItem[] {
    const raw = this.getAttribute("items");
    if (!raw) return [];
    try {
      const parsed = JSON.parse(raw);
      if (!Array.isArray(parsed)) return [];
      return parsed
        .map((item): LxMediaSwiperItem | null => {
          if (!item || typeof item !== "object") return null;
          const rec = item as Record<string, unknown>;
          const type = rec.type === "video" ? "video" : rec.type === "image" ? "image" : null;
          const src = typeof rec.src === "string" ? rec.src.trim() : "";
          if (!type || !src) return null;
          if (type === "image") {
            return { id: typeof rec.id === "string" ? rec.id : undefined, type, src };
          }
          return {
            id: typeof rec.id === "string" ? rec.id : undefined,
            type,
            src,
            poster: typeof rec.poster === "string" ? rec.poster : undefined,
            controls: typeof rec.controls === "boolean" ? rec.controls : undefined,
            muted: typeof rec.muted === "boolean" ? rec.muted : undefined,
          };
        })
        .filter((item): item is LxMediaSwiperItem => item !== null);
    } catch {
      return [];
    }
  }

  private parseDots(): false | { color?: string; activeColor?: string } {
    if (!this.hasAttribute("dots")) return false;
    const raw = this.getAttribute("dots");
    if (!raw || raw === "true") return {};
    if (raw === "false") return false;
    try {
      const parsed = JSON.parse(raw);
      if (!parsed || typeof parsed !== "object") return {};
      const rec = parsed as Record<string, unknown>;
      return {
        color: typeof rec.color === "string" ? rec.color : undefined,
        activeColor: typeof rec.activeColor === "string" ? rec.activeColor : undefined,
      };
    } catch {
      return {};
    }
  }

  private parseIntegerAttr(name: string): number | undefined {
    const raw = this.getAttribute(name);
    if (raw == null || raw.trim() === "") return undefined;
    const parsed = Number(raw);
    return Number.isFinite(parsed) ? Math.trunc(parsed) : undefined;
  }

  private parseRotate(value: unknown): 0 | 90 | 180 | 270 | undefined {
    const parsed = typeof value === "number" ? value : typeof value === "string" ? Number(value.trim()) : NaN;
    return parsed === 0 || parsed === 90 || parsed === 180 || parsed === 270 ? parsed : undefined;
  }

  private parseObjectFit(value: unknown): ObjectFit | undefined {
    if (typeof value !== "string") return undefined;
    const normalized = value.trim().toLowerCase();
    return normalized === "cover" || normalized === "contain" || normalized === "fill" || normalized === "fit"
      ? normalized
      : undefined;
  }

  private parseAnimation(value: unknown): Animation {
    return value === "none" ? "none" : "slide";
  }

  private parseDirection(value: unknown): Direction {
    return value === "vertical" ? "vertical" : "horizontal";
  }

  /**
   * Parse the `peek` attribute into a normalised `{ previous, next }` shape.
   * Returns `undefined` when no peek is configured so native sides can fall back
   * to the zero-peek (full-width page) layout.
   */
  private parsePeek(): { previous: number; next: number } | undefined {
    if (!this.hasAttribute("peek")) return undefined;
    const raw = this.getAttribute("peek");
    if (raw == null) return undefined;
    const trimmed = raw.trim();
    if (trimmed.length === 0) return undefined;
    const numeric = Number(trimmed);
    if (Number.isFinite(numeric)) {
      const value = Math.max(0, numeric);
      return value > 0 ? { previous: value, next: value } : undefined;
    }
    try {
      const parsed = JSON.parse(trimmed);
      if (!parsed || typeof parsed !== "object") return undefined;
      const rec = parsed as Record<string, unknown>;
      const previous = Math.max(0, Number(rec.previous) || 0);
      const next = Math.max(0, Number(rec.next) || 0);
      return previous > 0 || next > 0 ? { previous, next } : undefined;
    } catch {
      return undefined;
    }
  }

  private collectDataset(): Record<string, string> {
    const dataset: Record<string, string> = {};
    for (const attr of this.getAttributeNames()) {
      if (!attr.startsWith("data-")) continue;
      const key = attr.slice(5).replace(/-([a-z])/g, (_, c: string) => c.toUpperCase());
      const value = this.getAttribute(attr);
      if (key && value != null) dataset[key] = value;
    }
    return dataset;
  }
}

export function registerMediaSwiperComponent() {
  if (!customElements.get("lx-media-swiper")) {
    customElements.define("lx-media-swiper", LxMediaSwiperElement);
  }
}
