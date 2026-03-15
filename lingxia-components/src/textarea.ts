import { sendNativeComponentMessage, registerNativeComponentHandler } from "./nativecomponent.js";
import { ensureComponentId, NativeComponentUpdateState } from "./component.js";
import { measureElement } from "./dom.js";
import { isHarmony, isDesktop, isMacOS, isAndroid, isIOS } from "./platform.js";
import {
  getPropOrAttr as getSharedPropOrAttr,
  getBoolAttr as getSharedBoolAttr,
  getNumAttr as getSharedNumAttr,
  parseNumberLike,
  collectPageFuncBindings as collectSharedPageFuncBindings,
  shouldRefreshForBindingAttribute as shouldRefreshSharedBindingAttribute,
  collectDataset as collectSharedDataset,
  dispatchLogicBinding as dispatchSharedLogicBinding,
  ensureElementVisibleForKeyboard
} from "./text_component_shared.js";

const HARMONY_PROPS_PREFIX = "data:application/json,";

// Type definitions
export interface LxTextareaEventDetail {
  value?: string;
  cursor?: number;
  lineCount?: number;
  height?: number;
  duration?: number;
  selectionStart?: number;
  selectionEnd?: number;
  heightRpx?: number;
}

export interface LxTextareaEvent extends CustomEvent<LxTextareaEventDetail> {
  detail: LxTextareaEventDetail;
}

export type LxTextareaAttributes = {
  id?: string;
  value?: string;
  placeholder?: string;
  'placeholder-style'?: string;
  'placeholder-class'?: string;
  maxlength?: number | string;
  disabled?: boolean | string;
  'auto-focus'?: boolean | string;
  focus?: boolean | string;
  'auto-height'?: boolean | string;
  'cursor-spacing'?: number | string;
  'show-confirm-bar'?: boolean | string;
  'adjust-position'?: boolean | string;
  'hold-keyboard'?: boolean | string;
  'disable-default-padding'?: boolean | string;
  'confirm-type'?: 'send' | 'search' | 'next' | 'go' | 'done' | 'return';
  'confirm-hold'?: boolean | string;
  fixed?: boolean | string;
  'adjust-keyboard-to'?: 'cursor' | 'bottom';
  cursor?: number | string;
  'selection-start'?: number | string;
  'selection-end'?: number | string;
  className?: string;
  style?: any;
  ref?: any;
  onInput?: (e: LxTextareaEvent) => void;
  onChange?: (e: LxTextareaEvent) => void;
  onFocus?: (e: LxTextareaEvent) => void;
  onBlur?: (e: LxTextareaEvent) => void;
  onConfirm?: (e: LxTextareaEvent) => void;
  onLinechange?: (e: LxTextareaEvent) => void;
  onKeyboardheightchange?: (e: LxTextareaEvent) => void;
  bindInput?: string;
  bindChange?: string;
  bindFocus?: string;
  bindBlur?: string;
  bindConfirm?: string;
  bindLinechange?: string;
  bindKeyboardheightchange?: string;
  catchInput?: string;
  catchChange?: string;
  catchFocus?: string;
  catchBlur?: string;
  catchConfirm?: string;
  catchLinechange?: string;
  catchKeyboardheightchange?: string;
};

declare global {
  namespace JSX {
    interface IntrinsicElements {
      "lx-textarea": LxTextareaAttributes;
    }
  }
}

// Component implementation
export class LxTextareaElement extends HTMLElement {
  static get observedAttributes() {
    return [
      "id", "value", "placeholder", "placeholder-style",
      "placeholder-class", "maxlength", "disabled", "auto-focus", "focus",
      "auto-height", "cursor-spacing", "show-confirm-bar", "adjust-position",
      "hold-keyboard", "disable-default-padding", "confirm-type",
      "confirm-hold", "fixed", "adjust-keyboard-to", "cursor",
      "selection-start", "selection-end"
    ];
  }

  private componentId: string | null = null;
  private mounted = false;
  private unregister?: () => void;
  private _handlers: Record<string, EventListenerOrEventListenerObject> = {};
  private rawHandlers: Record<string, EventListenerOrEventListenerObject> = {};
  private harmonyEmbed?: HTMLEmbedElement;
  private lastHarmonyProps?: string;
  private pendingHarmonyProps?: Record<string, unknown>;
  private pendingHarmonyRetryTimer: number | null = null;
  private pendingHarmonyRetryCount = 0;
  private attrObserver?: MutationObserver;
  private shadow?: ShadowRoot;
  private innerTextarea?: HTMLTextAreaElement;
  private lastNativeAutoHeight?: number;
  private autoHeightMinHeight?: number;
  private lastKeyboardHeight = 0;
  private readonly updateState = new NativeComponentUpdateState();
  private resizeObserver?: ResizeObserver;
  private pendingLayoutFrame: number | null = null;
  private focusLayoutSyncTimer: number | null = null;
  private readonly layoutEventListener: EventListenerObject = {
    handleEvent: () => this.schedulePositionSync()
  };

  private upgradeProperty(propName: string): void {
    const self = this as unknown as Record<string, unknown>;
    if (!Object.prototype.hasOwnProperty.call(self, propName)) return;
    const value = self[propName];
    delete self[propName];
    self[propName] = value;
  }

  private registerNativeHandler(): void {
    if (!this.componentId) return;
    if (this.unregister) {
      this.unregister();
      this.unregister = undefined;
    }
    this.unregister = registerNativeComponentHandler(this.componentId, (message) => {
      if (typeof message.event !== "string") return;
      const detail = this.normalizeNativeDetail(message.detail) as LxTextareaEventDetail;
      if (isHarmony()) {
        this.dispatchLogicBinding(message.event, detail);
      }
      if (message.event === "linechange") {
        this.applyNativeAutoHeightIfNeeded(detail);
      } else if (message.event === "keyboardheightchange") {
        const nextKeyboardHeight = this.parseHeight(detail.height);
        if (nextKeyboardHeight !== undefined) {
          this.lastKeyboardHeight = Math.max(0, nextKeyboardHeight);
        }
        this.resyncHarmonyLayoutAfterFocus();
        this.ensureVisibleForKeyboard(this.lastKeyboardHeight, false);
      } else if (message.event === "focus") {
        this.startFocusLayoutSync();
        this.resyncHarmonyLayoutAfterFocus();
      } else if (message.event === "blur") {
        this.lastKeyboardHeight = 0;
        this.stopFocusLayoutSync();
      }
      this.dispatchEvent(new CustomEvent(message.event, { detail, bubbles: true }));
    });
  }

  private normalizeNativeDetail(detail: unknown): unknown {
    if (!detail || typeof detail !== "object") {
      return detail ?? {};
    }
    const raw = detail as Record<string, unknown>;
    if (raw.data && typeof raw.data === "object") {
      const data = raw.data as Record<string, unknown>;
      if (
        "value" in data ||
        "cursor" in data ||
        "selectionStart" in data ||
        "selectionEnd" in data ||
        "lineCount" in data ||
        "height" in data ||
        "heightRpx" in data ||
        "duration" in data
      ) {
        return data;
      }
    }
    return raw;
  }

  private unmountNativeComponentById(componentId: string): void {
    const useDesktopFallback = isDesktop() && !isMacOS();
    if (!componentId || isHarmony() || useDesktopFallback) return;
    sendNativeComponentMessage({
      action: "component.unmount",
      id: componentId
    });
  }

  private teardownHarmonyEmbed(): void {
    if (this.harmonyEmbed && this.contains(this.harmonyEmbed)) {
      this.removeChild(this.harmonyEmbed);
    }
    this.harmonyEmbed = undefined;
    this.lastHarmonyProps = undefined;
    if (this.pendingHarmonyRetryTimer !== null) {
      clearTimeout(this.pendingHarmonyRetryTimer);
      this.pendingHarmonyRetryTimer = null;
    }
    this.pendingHarmonyProps = undefined;
    this.pendingHarmonyRetryCount = 0;
  }

  connectedCallback() {
    this.upgradeProperty("oninput");
    this.upgradeProperty("onchange");
    this.upgradeProperty("onfocus");
    this.upgradeProperty("onblur");
    this.upgradeProperty("onconfirm");
    this.upgradeProperty("onlinechange");
    this.upgradeProperty("onkeyboardheightchange");

    this.componentId = ensureComponentId(this, "lx-textarea", this.componentId);
    if (!this.componentId) return;
    this.registerNativeHandler();

    this.startAttrObserver();
    this.startTracking();
    this.mountTextarea();
    queueMicrotask(() => {
      if (!this.isConnected) return;
      this.mountTextarea();
    });
  }

  disconnectedCallback() {
    if (this.shadow) {
      this.innerTextarea = undefined;
      this.shadow = undefined;
    }

    if (this.componentId) {
      this.unmountNativeComponentById(this.componentId);
    }
    if (this.unregister) {
      this.unregister();
      this.unregister = undefined;
    }
    this.teardownHarmonyEmbed();
    this.lastNativeAutoHeight = undefined;
    this.autoHeightMinHeight = undefined;
    this.lastKeyboardHeight = 0;
    this.stopFocusLayoutSync();
    this.stopTracking();
    this.mounted = false;
    this.updateState.reset();
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
      this.componentId = ensureComponentId(this, "lx-textarea", this.componentId);
      if (prev && this.componentId !== prev) {
        this.mounted = false;
        this.updateState.reset();
        if (this.isConnected) {
          this.unmountNativeComponentById(prev);
          this.teardownHarmonyEmbed();
          this.registerNativeHandler();
        }
      }
    }
    if (!this.isConnected) return;
    this.mountTextarea();
  }

  private getAttr(name: string, fallback = "") {
    const raw = this.getPropOrAttr(name);
    if (raw === undefined || raw === null) return fallback;
    return String(raw);
  }

  private getBoolAttr(name: string): boolean {
    return getSharedBoolAttr(this, name);
  }

  private getNumAttr(name: string): number | undefined {
    return getSharedNumAttr(this, name);
  }

  private getPropOrAttr(name: string): unknown {
    return getSharedPropOrAttr(this, name);
  }

  private shouldTrackNativeLayout(): boolean {
    return isAndroid() || isIOS() || isHarmony();
  }

  private startTracking(): void {
    if (!this.shouldTrackNativeLayout()) return;
    window.addEventListener("resize", this.layoutEventListener);
    if (isAndroid()) {
      // Android overlay can drift inside nested scrolling containers; capture phase catches element scroll events.
      window.addEventListener("scroll", this.layoutEventListener, { capture: true, passive: true });
    } else if (isHarmony()) {
      // Harmony native embed coordinates need continuous sync while page scrolls.
      window.addEventListener("scroll", this.layoutEventListener, { capture: true, passive: true });
    } else if (isIOS()) {
      // Match video behavior on iOS: track top-level page scrolling only.
      window.addEventListener("scroll", this.layoutEventListener, { passive: true });
    }
    window.visualViewport?.addEventListener("resize", this.layoutEventListener);
    window.visualViewport?.addEventListener("scroll", this.layoutEventListener);
    this.startSizeObserver();
    this.schedulePositionSync();
  }

  private stopTracking(): void {
    window.removeEventListener("resize", this.layoutEventListener);
    window.removeEventListener("scroll", this.layoutEventListener, true);
    window.removeEventListener("scroll", this.layoutEventListener);
    window.visualViewport?.removeEventListener("resize", this.layoutEventListener);
    window.visualViewport?.removeEventListener("scroll", this.layoutEventListener);
    this.stopSizeObserver();
    if (this.pendingLayoutFrame !== null) {
      cancelAnimationFrame(this.pendingLayoutFrame);
      this.pendingLayoutFrame = null;
    }
  }

  private startSizeObserver(): void {
    if (typeof ResizeObserver === "undefined" || this.resizeObserver) return;
    this.resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        if (entry.target !== this) continue;
        this.schedulePositionSync();
        break;
      }
    });
    this.resizeObserver.observe(this);
  }

  private stopSizeObserver(): void {
    if (!this.resizeObserver) return;
    this.resizeObserver.disconnect();
    this.resizeObserver = undefined;
  }

  private schedulePositionSync(): void {
    if (!this.shouldTrackNativeLayout() || !this.isConnected) return;
    if (this.pendingLayoutFrame !== null) return;
    this.pendingLayoutFrame = requestAnimationFrame(() => {
      this.pendingLayoutFrame = null;
      if (!this.isConnected) return;
      this.mountTextarea();
    });
  }

  private applyNativeAutoHeightIfNeeded(detail: LxTextareaEventDetail): void {
    if (!this.getBoolAttr("auto-height")) {
      this.autoHeightMinHeight = undefined;
      return;
    }
    const height = this.parseHeight(detail.height ?? detail.heightRpx);
    if (height === undefined || height <= 0) return;
    // iOS/Harmony should follow content height directly.
    // Android keeps a floor to avoid visible jump during first few layout passes.
    let nextHeight = height;
    if (isAndroid()) {
      if (this.autoHeightMinHeight === undefined) {
        const currentHeight = this.getBoundingClientRect().height;
        if (Number.isFinite(currentHeight) && currentHeight > 1) {
          this.autoHeightMinHeight = currentHeight;
        }
      }
      if (this.autoHeightMinHeight !== undefined) {
        nextHeight = Math.max(nextHeight, this.autoHeightMinHeight);
      }
    } else {
      this.autoHeightMinHeight = undefined;
    }
    if (this.lastNativeAutoHeight !== undefined && Math.abs(this.lastNativeAutoHeight - nextHeight) < 0.5) return;
    this.lastNativeAutoHeight = nextHeight;
    this.style.height = `${nextHeight}px`;
    this.mountTextarea();
    requestAnimationFrame(() => {
      if (!this.isConnected) return;
      this.mountTextarea();
    });
    setTimeout(() => {
      if (!this.isConnected) return;
      this.mountTextarea();
    }, 32);
  }

  private parseHeight(value: unknown): number | undefined {
    return parseNumberLike(value);
  }

  private collectProps() {
    const autoFocus = this.getBoolAttr("auto-focus");
    const value = this.getPropOrAttr("value");
    const props: Record<string, any> = {
      value: value === undefined || value === null ? undefined : String(value),
      placeholder: this.getAttr("placeholder"),
      placeholderStyle: this.getAttr("placeholder-style"),
      disabled: this.getBoolAttr("disabled"),
      // Use string values so "false" survives all bridge layers (some paths may drop falsy booleans).
      focus: (this.getBoolAttr("focus") || autoFocus) ? "true" : "false",
      autoFocus,
      autoHeight: this.getBoolAttr("auto-height"),
      showConfirmBar: this.getAttribute("show-confirm-bar") !== "false",
      adjustPosition: this.getAttribute("adjust-position") !== "false",
      holdKeyboard: this.getBoolAttr("hold-keyboard"),
      disableDefaultPadding: this.getBoolAttr("disable-default-padding"),
      confirmHold: this.getBoolAttr("confirm-hold"),
      fixed: this.getBoolAttr("fixed"),
    };

    const textColor = this.getHostTextColor();
    if (textColor) props.textColor = textColor;

    const placeholderClass = this.getAttr("placeholder-class");
    if (placeholderClass) props.placeholderClass = placeholderClass;

    const maxlength = this.getNumAttr("maxlength");
    if (maxlength !== undefined) props.maxlength = maxlength;

    const cursorSpacing = this.getNumAttr("cursor-spacing");
    if (cursorSpacing !== undefined) props.cursorSpacing = cursorSpacing;

    const confirmType = this.getAttr("confirm-type");
    if (confirmType) props.confirmType = confirmType;

    const adjustKeyboardTo = this.getAttr("adjust-keyboard-to");
    if (adjustKeyboardTo) props.adjustKeyboardTo = adjustKeyboardTo;

    const cursor = this.getNumAttr("cursor");
    if (cursor !== undefined) props.cursor = cursor;

    const selectionStart = this.getNumAttr("selection-start");
    if (selectionStart !== undefined) props.selectionStart = selectionStart;

    const selectionEnd = this.getNumAttr("selection-end");
    if (selectionEnd !== undefined) props.selectionEnd = selectionEnd;

    return props;
  }

  private getHostTextColor(): string | undefined {
    const color = getComputedStyle(this).color?.trim();
    if (!color) return undefined;
    if (color === "transparent" || color === "rgba(0, 0, 0, 0)") return undefined;
    return color;
  }

  private shouldAdjustPosition(): boolean {
    return this.getAttribute("adjust-position") !== "false";
  }

  private ensureVisibleForKeyboard(explicitKeyboardHeight = 0, forceCenter = false): void {
    if (isHarmony()) return;
    if (!this.shouldAdjustPosition()) return;
    ensureElementVisibleForKeyboard(this, explicitKeyboardHeight, forceCenter, [120]);
  }

  private resyncHarmonyLayoutAfterFocus(): void {
    if (!isHarmony() || !this.isConnected) return;
    this.mountTextarea();
    setTimeout(() => {
      if (!this.isConnected) return;
      this.mountTextarea();
    }, 32);
    setTimeout(() => {
      if (!this.isConnected) return;
      this.mountTextarea();
    }, 96);
  }

  private startFocusLayoutSync(): void {
    if (!isHarmony()) return;
    if (this.focusLayoutSyncTimer !== null) return;
    this.focusLayoutSyncTimer = window.setInterval(() => {
      if (!this.isConnected) {
        this.stopFocusLayoutSync();
        return;
      }
      this.mountTextarea();
    }, 80);
  }

  private stopFocusLayoutSync(): void {
    if (this.focusLayoutSyncTimer === null) return;
    clearInterval(this.focusLayoutSyncTimer);
    this.focusLayoutSyncTimer = null;
  }

  private collectPageFuncBindings() {
    return collectSharedPageFuncBindings(this);
  }

  private shouldRefreshForAttribute(name: string): boolean {
    return shouldRefreshSharedBindingAttribute(name);
  }

  private startAttrObserver(): void {
    if (typeof MutationObserver === "undefined" || this.attrObserver) return;
    this.attrObserver = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        const attrName = mutation.attributeName;
        if (!attrName || !this.shouldRefreshForAttribute(attrName)) continue;
        this.mountTextarea();
        break;
      }
    });
    this.attrObserver.observe(this, { attributes: true });
  }

  private stopAttrObserver(): void {
    if (!this.attrObserver) return;
    this.attrObserver.disconnect();
    this.attrObserver = undefined;
  }

  private collectDataset(): Record<string, string> {
    return collectSharedDataset(this);
  }

  private dispatchLogicBinding(eventName: string, detail: LxTextareaEventDetail): void {
    dispatchSharedLogicBinding(this, this.componentId ?? "", eventName, detail, this.collectPageFuncBindings());
  }

  private autoResizeTextarea(textarea: HTMLTextAreaElement): void {
    textarea.style.height = "auto";
    textarea.style.height = `${textarea.scrollHeight}px`;
  }

  private getLineCount(textarea: HTMLTextAreaElement): number {
    const lineHeight = parseInt(getComputedStyle(textarea).lineHeight) || 20;
    return Math.round(textarea.scrollHeight / lineHeight);
  }

  private mountDesktopTextarea() {
    const autoHeight = this.getBoolAttr("auto-height");

    if (!this.shadow) {
      this.shadow = this.attachShadow({ mode: "open" });
      const style = document.createElement("style");
      style.textContent = `
        :host {
          display: block;
          width: 100%;
        }
        textarea {
          display: block;
          width: 100%;
          box-sizing: border-box;
          padding: 8px 12px;
          border: 1px solid #d1d5db;
          border-radius: 6px;
          font-size: 14px;
          line-height: 1.5;
          color: #111827;
          background: #fff;
          outline: none;
          transition: border-color 0.2s;
          resize: vertical;
          min-height: 80px;
          font-family: inherit;
        }
        textarea.auto-height {
          resize: none;
          overflow: hidden;
        }
        textarea:focus {
          border-color: #3b82f6;
          box-shadow: 0 0 0 2px rgba(59, 130, 246, 0.15);
        }
        textarea:disabled {
          background: #f3f4f6;
          color: #9ca3af;
          cursor: not-allowed;
        }
        textarea::placeholder {
          color: #9ca3af;
        }
      `;
      this.shadow.appendChild(style);

      const textarea = document.createElement("textarea");
      this.innerTextarea = textarea;
      this.shadow.appendChild(textarea);

      let lastLineCount = 0;

      // Stop native composed events from leaking through shadow boundary
      // to avoid double-firing (native retarget + our CustomEvent).
      textarea.addEventListener("input", (e) => {
        e.stopPropagation();
        const detail: LxTextareaEventDetail = { value: textarea.value };
        this.dispatchEvent(new CustomEvent("input", { detail, bubbles: true }));
        this.dispatchLogicBinding("input", detail);

        if (textarea.classList.contains("auto-height")) {
          this.autoResizeTextarea(textarea);
        }

        const lineCount = this.getLineCount(textarea);
        if (lineCount !== lastLineCount) {
          lastLineCount = lineCount;
          const lineDetail: LxTextareaEventDetail = { lineCount };
          this.dispatchEvent(new CustomEvent("linechange", { detail: lineDetail, bubbles: true }));
          this.dispatchLogicBinding("linechange", lineDetail);
        }
      });
      textarea.addEventListener("change", (e) => {
        e.stopPropagation();
        const detail: LxTextareaEventDetail = { value: textarea.value };
        this.dispatchEvent(new CustomEvent("change", { detail, bubbles: true }));
        this.dispatchLogicBinding("change", detail);
      });
      textarea.addEventListener("focus", (e) => {
        e.stopPropagation();
        const detail: LxTextareaEventDetail = { value: textarea.value };
        this.dispatchEvent(new CustomEvent("focus", { detail, bubbles: true }));
        this.dispatchLogicBinding("focus", detail);
      });
      textarea.addEventListener("blur", (e) => {
        e.stopPropagation();
        const detail: LxTextareaEventDetail = { value: textarea.value };
        this.dispatchEvent(new CustomEvent("blur", { detail, bubbles: true }));
        this.dispatchLogicBinding("blur", detail);
      });
      textarea.addEventListener("keydown", (e) => {
        if (e.key !== "Enter" || e.shiftKey) return;
        const confirmType = this.getAttr("confirm-type", "return").trim().toLowerCase();
        if (confirmType === "return") return;

        e.preventDefault();
        const detail: LxTextareaEventDetail = { value: textarea.value };
        this.dispatchEvent(new CustomEvent("confirm", { detail, bubbles: true }));
        this.dispatchLogicBinding("confirm", detail);
        if (!this.getBoolAttr("confirm-hold")) {
          textarea.blur();
        }
      });
    }

    // Sync attributes to inner textarea
    const textarea = this.innerTextarea!;
    const props = this.collectProps();
    textarea.placeholder = props.placeholder || "";
    textarea.disabled = props.disabled;
    if (props.maxlength !== undefined) {
      textarea.maxLength = props.maxlength;
    } else {
      textarea.removeAttribute("maxlength");
    }

    if (autoHeight) {
      textarea.classList.add("auto-height");
    } else {
      textarea.classList.remove("auto-height");
    }

    // Only sync value if it differs (avoid cursor jump)
    if (props.value !== undefined && textarea.value !== props.value) {
      textarea.value = props.value;
    }

    if (
      typeof props.selectionStart === "number" &&
      typeof props.selectionEnd === "number" &&
      props.selectionStart >= 0 &&
      props.selectionEnd >= props.selectionStart
    ) {
      textarea.setSelectionRange(props.selectionStart, props.selectionEnd);
    }

    // Apply placeholder style (replace on every update so dynamic changes work)
    const placeholderStyle = props.placeholderStyle;
    const styleEl = this.shadow!.querySelector("style")!;
    const marker = "/* placeholder-style */";
    const markerIdx = styleEl.textContent!.indexOf(marker);
    if (placeholderStyle) {
      const rule = `\n${marker}\ntextarea::placeholder { ${placeholderStyle} }`;
      if (markerIdx >= 0) {
        styleEl.textContent = styleEl.textContent!.substring(0, markerIdx) + rule;
      } else {
        styleEl.textContent += rule;
      }
    } else if (markerIdx >= 0) {
      styleEl.textContent = styleEl.textContent!.substring(0, markerIdx);
    }

    if (autoHeight) {
      this.autoResizeTextarea(textarea);
    }

    if (props.focus && document.activeElement !== textarea) {
      textarea.focus();
    }
  }

  private mountTextarea() {
    if (!this.componentId) return;
    this.ensureHostLayout();

    const props = this.collectProps();
    const dataset = this.collectDataset();
    props.dataset = dataset;
    props.datasetJson = JSON.stringify(dataset);
    const pageFuncBindings = this.collectPageFuncBindings() ?? {};
    props.pageFuncBindings = pageFuncBindings;
    props.pageFuncBindingsJson = JSON.stringify(pageFuncBindings);
    const { rect, cornerRadius } = measureElement(this);
    props.__layoutX = rect.x;
    props.__layoutY = rect.y;
    props.__layoutWidth = rect.width;
    props.__layoutHeight = rect.height;

    if (isHarmony()) {
      this.ensureHarmonyEmbed(rect, props, cornerRadius);
      this.mounted = true;
      return;
    }

    if (!this.getBoolAttr("auto-height")) {
      this.autoHeightMinHeight = undefined;
    }

    if (isDesktop() && !isMacOS()) {
      this.mountDesktopTextarea();
      this.mounted = true;
      return;
    }

    const zIndex = 9999;
    const decision = this.updateState.decide(rect, props, zIndex, !this.mounted);
    if (!decision.shouldSend) {
      this.mounted = true;
      return;
    }

    const payload: any = {
      action: this.mounted ? "component.update" : "component.mount",
      id: this.componentId,
      type: "textarea.native",
      rect,
      zIndex
    };
    if (!this.mounted || decision.propsChanged) {
      payload.props = props;
    }

    if (cornerRadius !== undefined) {
      payload.cornerRadius = cornerRadius;
    }

    sendNativeComponentMessage(payload);
    this.mounted = true;
  }

  // Force a full native sync when host frameworks mutate properties without
  // triggering observed attribute callbacks on custom elements.
  syncNativeProps(): void {
    if (!this.isConnected) return;
    this.mountTextarea();
  }

  private ensureHostLayout(): void {
    const computed = getComputedStyle(this);
    if (computed.display === "inline") {
      this.style.display = "block";
    }
    const height = Number.parseFloat(computed.height);
    if (!Number.isFinite(height) || height <= 1) {
      this.style.height = this.getBoolAttr("auto-height") ? "24px" : "96px";
    }
  }

  // Event handler getter/setter for 'input' event
  set oninput(cb: EventListener | null) {
    const raw = cb as unknown;
    const current = this._handlers['input'];
    if (current) this.removeEventListener('input', current);

    if (
      typeof raw === "function" ||
      (!!raw && typeof raw === "object" && typeof (raw as EventListenerObject).handleEvent === "function")
    ) {
      const listener =
        typeof raw === "function"
          ? ({ handleEvent: raw } as EventListenerObject)
          : (raw as EventListenerOrEventListenerObject);
      this._handlers['input'] = listener;
      this.rawHandlers['input'] = raw as EventListenerOrEventListenerObject;
      this.addEventListener('input', listener);
    } else {
      delete this._handlers['input'];
      delete this.rawHandlers['input'];
    }
  }
  get oninput() { return (this.rawHandlers['input'] as EventListener) || null; }

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
    if (!this.harmonyEmbed) {
      const embed = document.createElement("embed");
      embed.setAttribute("type", "native/textarea");
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
      this.pendingHarmonyRetryCount = 0;
      return;
    }

    const encodedProps = this.encodeHarmonyProps(props);
    if (encodedProps !== this.lastHarmonyProps) {
      const pushed = this.pushHarmonyPropsUpdate(props);
      if (pushed) {
        this.lastHarmonyProps = encodedProps;
        this.pendingHarmonyProps = undefined;
        this.pendingHarmonyRetryCount = 0;
      } else {
        // Fallback to src update path when proxy bridge is unavailable.
        this.harmonyEmbed.setAttribute("src", encodedProps);
        this.lastHarmonyProps = encodedProps;
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
      return `${HARMONY_PROPS_PREFIX}${encodeURIComponent(JSON.stringify(payload))}`;
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
}

// Register the custom element
export function registerTextareaComponent() {
  if (typeof window !== "undefined" && !customElements.get("lx-textarea")) {
    customElements.define("lx-textarea", LxTextareaElement);
  }
}
