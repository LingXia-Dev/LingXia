import { sendNativeComponentMessage, registerNativeComponentHandler } from "./nativecomponent.js";
import { ensureComponentId } from "./component.js";
import { measureElement } from "./dom.js";
import { isHarmony, isDesktop } from "./platform.js";

const HARMONY_PROPS_PREFIX = "data:application/json,";

// Type definitions
export type LxPickerColumn = string[];
export type LxPickerCascadingColumns = [string[], Record<string, string[]>];
type LxPickerViewEventHandler = (e: LxPickerEvent) => void;
type LxPickerLogicEventHandler = string;

export interface LxPickerEventDetail {
  index?: number | number[];
  value?: string | string[];
  confirmed?: boolean;
  cancelled?: boolean;
}

export interface LxPickerEvent extends CustomEvent<LxPickerEventDetail> {
  detail: LxPickerEventDetail;
}

export type LxPickerAttributes = {
  id?: string;
  mode?: 'selector' | 'multiSelector' | 'cascading' | 'date' | 'time';
  columns?: LxPickerColumn[] | LxPickerCascadingColumns;
  defaultIndex?: number | number[];
  value?: string;
  start?: string;
  end?: string;
  fields?: 'year' | 'month' | 'day' | 'range';
  cancelText?: string;
  confirmText?: string;
  cancelButtonColor?: string;
  confirmButtonColor?: string;
  cancelTextColor?: string;
  confirmTextColor?: string;
  className?: string;
  style?: any;
  ref?: any;
  onChange?: LxPickerViewEventHandler;
  onScroll?: LxPickerViewEventHandler;
  bindChange?: LxPickerLogicEventHandler;
  bindScroll?: LxPickerLogicEventHandler;
  catchChange?: LxPickerLogicEventHandler;
  catchScroll?: LxPickerLogicEventHandler;
};

declare global {
  namespace JSX {
    interface IntrinsicElements {
      "lx-picker": LxPickerAttributes;
    }
  }
}

// Component implementation
export class LxPickerElement extends HTMLElement {
  static get observedAttributes() {
    return [
      "id",
      "mode",
      "columns",
      "default-index",
      "value",
      "start",
      "end",
      "fields",
      "cancel-text",
      "confirm-text",
      "cancel-button-color",
      "confirm-button-color",
      "cancel-text-color",
      "confirm-text-color"
    ];
  }

  private componentId: string | null = null;
  private mounted = false;
  private unregister?: () => void;
  private _handlers: Record<string, EventListenerOrEventListenerObject> = {};
  private rawHandlers: Record<string, EventListenerOrEventListenerObject> = {};
  private harmonyEmbed?: HTMLEmbedElement;
  private lastHarmonyProps?: string;
  private webCleanup?: () => void;
  private attrObserver?: MutationObserver;
  private desktopScrollListener?: EventListenerObject;

  private upgradeProperty(propName: string): void {
    const self = this as unknown as Record<string, unknown>;
    if (!Object.prototype.hasOwnProperty.call(self, propName)) return;
    const value = self[propName];
    delete self[propName];
    self[propName] = value;
  }

  connectedCallback() {
    this.upgradeProperty("onchange");

    this.componentId = ensureComponentId(this, "lx-picker", this.componentId);
    if (!this.componentId) return;

    this.unregister = registerNativeComponentHandler(this.componentId!, (message) => {
      if (typeof message.event === "string") {
        this.dispatchEvent(new CustomEvent(message.event, { detail: message.detail ?? {}, bubbles: true }));
      }
    });

    this.startAttrObserver();
    this.mountPicker();
    queueMicrotask(() => {
      if (!this.isConnected) return;
      this.mountPicker();
    });
  }

  disconnectedCallback() {
    if (this.webCleanup) {
      this.webCleanup();
      this.webCleanup = undefined;
    }
    if (this.desktopScrollListener) {
      this.removeEventListener('scroll', this.desktopScrollListener);
      this.desktopScrollListener = undefined;
    }

    if (this.componentId && !isHarmony() && !isDesktop()) {
      sendNativeComponentMessage({
        action: "component.unmount",
        id: this.componentId
      });
    }
    if (this.unregister) {
      this.unregister();
      this.unregister = undefined;
    }
    if (this.harmonyEmbed && this.contains(this.harmonyEmbed)) {
      this.removeChild(this.harmonyEmbed);
    }
    this.harmonyEmbed = undefined;
    this.lastHarmonyProps = undefined;
    this.mounted = false;
    this.stopAttrObserver();
    Object.keys(this._handlers).forEach(name => {
      this.removeEventListener(name, this._handlers[name]);
    });
    this._handlers = {};
    this.rawHandlers = {};
  }

  attributeChangedCallback(name: string) {
    if (name === "id") {
      const prev = this.componentId;
      this.componentId = ensureComponentId(this, "lx-picker", this.componentId);
      if (prev && this.componentId !== prev) {
        this.mounted = false;
      }
    }
    if (!this.isConnected) return;
    this.mountPicker();
  }

  private getAttr(name: string, fallback = "") {
    return this.getAttribute(name) || fallback;
  }

  private getJsonAttr(name: string) {
    const raw = this.getAttribute(name);
    if (!raw) return undefined;
    try { return JSON.parse(raw); } catch { return undefined; }
  }

  private collectProps() {
    const mode = this.getAttr("mode", "selector");
    const isDateMode = mode === "date" || mode === "time";
    const fields = this.getAttr("fields");

    const props: Record<string, any> = {
      mode,
      cancelText: this.getAttr("cancel-text", ""),
      confirmText: this.getAttr("confirm-text", ""),
      cancelButtonColor: this.getAttr("cancel-button-color"),
      confirmButtonColor: this.getAttr("confirm-button-color"),
      cancelTextColor: this.getAttr("cancel-text-color"),
      confirmTextColor: this.getAttr("confirm-text-color")
    };

    if (isDateMode) {
      if (fields) props.fields = fields;
      const value = this.getAttr("value");
      if (value) {
        props.value = fields === "range" ? this.getJsonAttr("value") : value;
      }

      const start = this.getAttr("start");
      if (start) props.start = start;

      const end = this.getAttr("end");
      if (end) props.end = end;
    } else {
      props.columns = this.getJsonAttr("columns");
      props.defaultIndex = this.getJsonAttr("default-index");
    }

    return props;
  }

  private normalizeBindingEventKey(rawKey: string): string | null {
    const key = rawKey.trim().toLowerCase().replace(/[\-_:]/g, "");
    return key.length > 0 ? key : null;
  }

  private extractBindingEventKey(attrName: string): string | null {
    const attr = attrName.trim().toLowerCase();
    let suffix: string | null = null;
    if (attr.startsWith("bind") && attr.length > 4) {
      suffix = attr.slice(4);
    } else if (attr.startsWith("catch") && attr.length > 5) {
      suffix = attr.slice(5);
    }
    if (!suffix) return null;
    return this.normalizeBindingEventKey(suffix.replace(/^[:\-]/, ""));
  }

  private collectPageFuncBindings() {
    const bindings: Record<string, string> = {};
    const attrs = this.getAttributeNames();
    for (const attr of attrs) {
      const eventKey = this.extractBindingEventKey(attr);
      if (!eventKey) continue;
      const funcName = this.getAttribute(attr)?.trim();
      if (!funcName) continue;
      bindings[eventKey] = funcName;
    }
    return Object.keys(bindings).length > 0 ? bindings : undefined;
  }

  private shouldRefreshForAttribute(name: string): boolean {
    const normalized = name.trim().toLowerCase();
    return normalized.startsWith("bind") || normalized.startsWith("catch") || normalized.startsWith("data-");
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
        this.mountPicker();
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

  private buildPageFuncEvent(eventName: string, detail: LxPickerEventDetail): Record<string, unknown> {
    const dataset = this.collectDataset();
    const target = {
      id: this.componentId,
      dataset
    };
    return {
      type: eventName,
      detail: detail ?? {},
      target,
      currentTarget: target,
      timeStamp: Date.now()
    };
  }

  private dispatchLogicBinding(eventName: string, detail: LxPickerEventDetail): void {
    const normalizedEvent = this.normalizeBindingEventKey(eventName);
    if (!normalizedEvent) {
      return;
    }
    const bindings = this.collectPageFuncBindings();
    const functionName = bindings?.[normalizedEvent];
    if (!functionName) {
      return;
    }
    const bridge = (window as Window & {
      LingXiaBridge?: { notify?: (method: string, params?: unknown) => void };
    }).LingXiaBridge;
    if (!bridge || typeof bridge.notify !== 'function') {
      return;
    }
    const payload = this.buildPageFuncEvent(normalizedEvent, detail);
    try {
      bridge.notify(functionName, payload);
    } catch {
      // Ignore notify errors to keep picker UX responsive.
    }
  }

  private async mountPicker() {
    if (!this.componentId) return;

    const props = this.collectProps();
    const dataset = this.collectDataset();
    props.dataset = dataset;
    props.datasetJson = JSON.stringify(dataset);
    const pageFuncBindings = this.collectPageFuncBindings() ?? {};
    props.pageFuncBindings = pageFuncBindings;
    props.pageFuncBindingsJson = JSON.stringify(pageFuncBindings);
    const { rect, cornerRadius } = measureElement(this);

    if (isHarmony()) {
      this.ensureHarmonyEmbed(rect, props, cornerRadius);
      this.mounted = true;
      return;
    }

    if (isDesktop()) {
      if (this.webCleanup) return;
      const { renderWebPicker } = await import('./picker-web.js');
      if (!this.isConnected || !this.componentId || this.webCleanup) return;
      const scrollListener: EventListenerObject = {
        handleEvent: (event: Event): void => {
          const detail = ((event as CustomEvent).detail ?? {}) as LxPickerEventDetail;
          this.dispatchLogicBinding('scroll', detail);
        }
      };
      this.desktopScrollListener = scrollListener;
      this.addEventListener('scroll', scrollListener);
      this.webCleanup = renderWebPicker(this, props, (detail) => {
        this.dispatchEvent(new CustomEvent('change', { detail, bubbles: true }));
        this.dispatchLogicBinding('change', detail);
      });
      this.mounted = true;
      return;
    }

    const payload: any = {
      action: this.mounted ? "component.update" : "component.mount",
      id: this.componentId,
      type: "picker.native",
      rect,
      zIndex: 9999,
      props: props
    };

    if (cornerRadius !== undefined) {
      payload.cornerRadius = cornerRadius;
    }

    sendNativeComponentMessage(payload);
    this.mounted = true;
  }

  // Event handler getter/setter for 'change' event
  set onchange(cb: EventListener | null) {
    const raw = cb as unknown;
    const current = this._handlers['change'];
    if (current) this.removeEventListener('change', current);

    if (
      typeof raw === "function" ||
      (!!raw && typeof raw === "object" && typeof (raw as EventListenerObject).handleEvent === "function")
    ) {
      const listener =
        typeof raw === "function"
          ? ({ handleEvent: raw } as EventListenerObject)
          : (raw as EventListenerOrEventListenerObject);
      this._handlers['change'] = listener;
      this.rawHandlers['change'] = raw as EventListenerOrEventListenerObject;
      this.addEventListener('change', listener);
    } else {
      delete this._handlers['change'];
      delete this.rawHandlers['change'];
    }
  }
  get onchange() { return (this.rawHandlers['change'] as EventListener) || null; }

  private ensureHarmonyEmbed(
    rect: { width: number; height: number },
    props: Record<string, unknown>,
    cornerRadius?: number
  ) {
    // Create embed element only once
    if (!this.harmonyEmbed) {
      const embed = document.createElement("embed");
      embed.setAttribute("type", "native/picker");
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

      this.appendChild(embed);
      this.harmonyEmbed = embed;
      return;
    }

    // Only update props via src if they actually changed
    const encodedProps = this.encodeHarmonyProps(props);
    if (encodedProps !== this.lastHarmonyProps) {
      this.harmonyEmbed.setAttribute("src", encodedProps);
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

// Register the custom element
export function registerPickerComponent() {
  if (typeof window !== "undefined" && !customElements.get("lx-picker")) {
    customElements.define("lx-picker", LxPickerElement);
  }
}
