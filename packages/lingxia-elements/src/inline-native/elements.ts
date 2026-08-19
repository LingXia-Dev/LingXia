import {
  collectAuthorTreeFromElement,
  compileInlineNativeRoot,
  parseBooleanAttr,
} from "./structure.js";
import type { CompileInlineNativeResult, NativeError, RootRef } from "./types.js";
import { nativeError } from "./errors.js";
import { identifyCompiledRoot, nextOpaqueKey, type IdentifiedRoot } from "./identity.js";
import { buildRootCommit } from "./commit.js";
import { sendNativeComponentMessage } from "../nativecomponent.js";

const ROOT_INVALID_EVENT = "error";
const STRUCTURE_COMPILED_EVENT = "lxnativecompiled";

type PendingCompile = {
  frame: number | null;
};

function asRecord(el: HTMLElement): HTMLElement & Record<string, unknown> {
  return el as HTMLElement & Record<string, unknown>;
}

function reflectBoolean(el: HTMLElement, name: string, value: unknown): void {
  if (value === false || value === "false") {
    el.setAttribute(name, "false");
    return;
  }
  if (value === true || value === "" || value === "true") {
    el.setAttribute(name, "");
    return;
  }
  if (value == null) {
    el.removeAttribute(name);
    return;
  }
  el.setAttribute(name, String(value));
}

function reflectString(el: HTMLElement, name: string, value: unknown): void {
  if (value == null || value === "") {
    el.removeAttribute(name);
    return;
  }
  el.setAttribute(name, String(value));
}

class LxNativeBaseElement extends HTMLElement {
  static get observedAttributes(): string[] {
    return [
      "id",
      "automation-id",
      "hidden",
      "hidden-transition",
      "pointer-events",
      "aria-label",
      "aria-description",
      "aria-hidden",
    ];
  }

  get automationId(): string | null {
    return this.getAttribute("automation-id");
  }
  set automationId(value: string | null | undefined) {
    reflectString(this, "automation-id", value);
  }

  get pointerEvents(): string | null {
    return this.getAttribute("pointer-events");
  }
  set pointerEvents(value: string | null | undefined) {
    reflectString(this, "pointer-events", value);
  }

  get hiddenTransition(): string | null {
    return this.getAttribute("hidden-transition");
  }
  set hiddenTransition(value: string | null | undefined) {
    reflectString(this, "hidden-transition", value);
  }
}

export class LxNativeRootElement extends LxNativeBaseElement {
  static get observedAttributes(): string[] {
    return [...LxNativeBaseElement.observedAttributes, "fullscreen-scope"];
  }

  private pending: PendingCompile = { frame: null };
  private observer?: MutationObserver;
  private lastResult: CompileInlineNativeResult | null = null;
  private rootKey = nextOpaqueKey("root");
  private rootEpoch = 1;
  private treeRevision = 0;
  private identified: IdentifiedRoot | null = null;

  get fullscreenScope(): string {
    return this.getAttribute("fullscreen-scope") ?? "root";
  }
  set fullscreenScope(value: string | null | undefined) {
    reflectString(this, "fullscreen-scope", value ?? "root");
  }

  lastCompileResult(): CompileInlineNativeResult | null {
    return this.lastResult;
  }

  retry(): Promise<void> {
    this.compileNow();
    return Promise.resolve();
  }

  connectedCallback(): void {
    this.style.display = this.style.display || "block";
    this.style.position = this.style.position || "relative";
    this.observer = new MutationObserver(() => this.scheduleCompile());
    this.observer.observe(this, { childList: true, subtree: true, attributes: true });
    this.scheduleCompile();
  }

  disconnectedCallback(): void {
    this.observer?.disconnect();
    this.observer = undefined;
    if (this.pending.frame != null && typeof cancelAnimationFrame === "function") {
      cancelAnimationFrame(this.pending.frame);
      this.pending.frame = null;
    }
  }

  private scheduleCompile(): void {
    if (typeof requestAnimationFrame !== "function") {
      this.compileNow();
      return;
    }
    if (this.pending.frame != null) return;
    this.pending.frame = requestAnimationFrame(() => {
      this.pending.frame = null;
      this.compileNow();
    });
  }

  compileNow(): CompileInlineNativeResult {
    const author = collectAuthorTreeFromElement(this);
    author.props = { ...(author.props ?? {}), fullscreenScope: this.fullscreenScope };
    const result = compileInlineNativeRoot(author);
    this.lastResult = result;
    if (!result.ok) {
      this.dispatchEvent(
        new CustomEvent(ROOT_INVALID_EVENT, {
          detail: result.error,
          bubbles: false,
        })
      );
    } else {
      const rootRef = this.rootRef();
      const identified = identifyCompiledRoot(result.root, rootRef, this.identified);
      this.treeRevision += 1;
      const commit = buildRootCommit(identified, this.identified, this.treeRevision);
      this.identified = identified;
      sendNativeComponentMessage({ id: this.rootKey, ...commit });
      this.dispatchEvent(
        new CustomEvent(STRUCTURE_COMPILED_EVENT, {
          detail: result.root,
          bubbles: false,
        })
      );
    }
    return result;
  }

  private rootRef(): RootRef {
    return pageRootScope(this.rootKey, this.rootEpoch);
  }
}

export class LxNativeViewElement extends LxNativeBaseElement {
  static get observedAttributes(): string[] {
    return [...LxNativeBaseElement.observedAttributes, "role"];
  }
}

export class LxNativeCoverElement extends LxNativeBaseElement {
  static get observedAttributes(): string[] {
    return [...LxNativeBaseElement.observedAttributes, "scrim", "scrim-opacity", "role"];
  }

  connectedCallback(): void {
    if (!this.style.position) this.style.position = "absolute";
    if (!this.style.inset && !this.style.top && !this.style.left) {
      this.style.inset = "0";
    }
    if (!this.style.pointerEvents) {
      this.style.pointerEvents = "none";
    }
  }

  get scrim(): string {
    return this.getAttribute("scrim") ?? "none";
  }
  set scrim(value: string | null | undefined) {
    reflectString(this, "scrim", value);
  }

  get scrimOpacity(): number {
    const raw = this.getAttribute("scrim-opacity");
    const parsed = raw == null ? 0.6 : Number(raw);
    return Number.isFinite(parsed) ? parsed : 0.6;
  }
  set scrimOpacity(value: number | string | null | undefined) {
    if (value == null) {
      this.removeAttribute("scrim-opacity");
      return;
    }
    this.setAttribute("scrim-opacity", String(value));
  }
}

export class LxNativeTextElement extends LxNativeBaseElement {
  static get observedAttributes(): string[] {
    return [
      ...LxNativeBaseElement.observedAttributes,
      "max-lines",
      "dir",
      "font-size",
      "font-weight",
      "line-height",
      "text-align",
      "color",
    ];
  }

  connectedCallback(): void {
    if (!this.style.pointerEvents) {
      this.style.pointerEvents = "none";
    }
  }

  get maxLines(): number | null {
    const raw = this.getAttribute("max-lines");
    if (raw == null) return null;
    const parsed = Number(raw);
    return Number.isFinite(parsed) ? parsed : null;
  }
  set maxLines(value: number | string | null | undefined) {
    reflectString(this, "max-lines", value == null ? null : String(value));
  }
}

export class LxNativeButtonElement extends LxNativeBaseElement {
  static get observedAttributes(): string[] {
    return [
      ...LxNativeBaseElement.observedAttributes,
      "label",
      "icon",
      "icon-position",
      "intent",
      "emphasis",
      "size",
      "hit-slop",
      "disabled",
      "pressed",
      "expanded",
      "loading",
    ];
  }

  get label(): string | null {
    return this.getAttribute("label");
  }
  set label(value: string | null | undefined) {
    reflectString(this, "label", value);
  }

  get icon(): string | null {
    return this.getAttribute("icon");
  }
  set icon(value: string | Record<string, unknown> | null | undefined) {
    if (value && typeof value === "object") {
      asRecord(this).__lxIconResource = value;
      this.removeAttribute("icon");
      return;
    }
    delete asRecord(this).__lxIconResource;
    reflectString(this, "icon", value);
  }

  get disabled(): boolean {
    return parseBooleanAttr(this.getAttribute("disabled"), false);
  }
  set disabled(value: unknown) {
    reflectBoolean(this, "disabled", value);
  }

  get pressed(): boolean {
    return parseBooleanAttr(this.getAttribute("pressed"), false);
  }
  set pressed(value: unknown) {
    reflectBoolean(this, "pressed", value);
  }

  get expanded(): boolean {
    return parseBooleanAttr(this.getAttribute("expanded"), false);
  }
  set expanded(value: unknown) {
    reflectBoolean(this, "expanded", value);
  }

  get loading(): boolean {
    return parseBooleanAttr(this.getAttribute("loading"), false);
  }
  set loading(value: unknown) {
    reflectBoolean(this, "loading", value);
  }
}

export class LxNativeSliderElement extends LxNativeBaseElement {
  static get observedAttributes(): string[] {
    return [
      ...LxNativeBaseElement.observedAttributes,
      "value",
      "min",
      "max",
      "step",
      "buffered-value",
      "value-label",
      "disabled",
    ];
  }

  get value(): number {
    const raw = this.getAttribute("value");
    const parsed = raw == null ? 0 : Number(raw);
    return Number.isFinite(parsed) ? parsed : 0;
  }
  set value(next: number | string | null | undefined) {
    reflectString(this, "value", next == null ? null : String(next));
  }

  get min(): number {
    const raw = this.getAttribute("min");
    const parsed = raw == null ? 0 : Number(raw);
    return Number.isFinite(parsed) ? parsed : 0;
  }
  set min(next: number | string | null | undefined) {
    reflectString(this, "min", next == null ? null : String(next));
  }

  get max(): number {
    const raw = this.getAttribute("max");
    const parsed = raw == null ? 100 : Number(raw);
    return Number.isFinite(parsed) ? parsed : 100;
  }
  set max(next: number | string | null | undefined) {
    reflectString(this, "max", next == null ? null : String(next));
  }

  get step(): number {
    const raw = this.getAttribute("step");
    const parsed = raw == null ? 0 : Number(raw);
    return Number.isFinite(parsed) ? parsed : 0;
  }
  set step(next: number | string | null | undefined) {
    reflectString(this, "step", next == null ? null : String(next));
  }

  get valueLabel(): string {
    return this.getAttribute("value-label") ?? "none";
  }
  set valueLabel(value: string | null | undefined) {
    reflectString(this, "value-label", value);
  }

  get disabled(): boolean {
    return parseBooleanAttr(this.getAttribute("disabled"), false);
  }
  set disabled(value: unknown) {
    reflectBoolean(this, "disabled", value);
  }
}

function pageRootScope(rootKey: string, rootEpoch: number): RootRef {
  let surfaceInstanceId = "surface";
  let pageInstanceId = "page";
  let documentInstanceId = "doc";
  if (typeof window !== "undefined") {
    const cfg = (window as Window & { __LX_BRIDGE_CFG?: Record<string, string> }).__LX_BRIDGE_CFG;
    if (cfg?.surfaceInstanceId) surfaceInstanceId = cfg.surfaceInstanceId;
    if (cfg?.pageInstanceId) pageInstanceId = cfg.pageInstanceId;
    const w = window as Window & { __LX_DOCUMENT_INSTANCE_ID?: string };
    if (!w.__LX_DOCUMENT_INSTANCE_ID) {
      w.__LX_DOCUMENT_INSTANCE_ID = nextOpaqueKey("doc");
    }
    documentInstanceId = w.__LX_DOCUMENT_INSTANCE_ID;
  }
  return { surfaceInstanceId, pageInstanceId, documentInstanceId, rootKey, rootEpoch };
}

function defineOnce(tag: string, ctor: CustomElementConstructor): void {
  if (typeof customElements === "undefined") return;
  if (customElements.get(tag)) return;
  customElements.define(tag, ctor);
}

export function registerNativeRootComponent(): void {
  defineOnce("lx-native-root", LxNativeRootElement);
}
export function registerNativeViewComponent(): void {
  defineOnce("lx-native-view", LxNativeViewElement);
}
export function registerNativeCoverComponent(): void {
  defineOnce("lx-native-cover", LxNativeCoverElement);
}
export function registerNativeTextComponent(): void {
  defineOnce("lx-native-text", LxNativeTextElement);
}
export function registerNativeButtonComponent(): void {
  defineOnce("lx-native-button", LxNativeButtonElement);
}
export function registerNativeSliderComponent(): void {
  defineOnce("lx-native-slider", LxNativeSliderElement);
}

export function registerInlineNativeAuthorComponents(): void {
  registerNativeRootComponent();
  registerNativeViewComponent();
  registerNativeCoverComponent();
  registerNativeTextComponent();
  registerNativeButtonComponent();
  registerNativeSliderComponent();
}

export function structureErrorFromUnknown(message: string): NativeError {
  return nativeError("NATIVE_ROOT_INVALID_STRUCTURE", message);
}

declare global {
  interface HTMLElementTagNameMap {
    "lx-native-root": LxNativeRootElement;
    "lx-native-view": LxNativeViewElement;
    "lx-native-cover": LxNativeCoverElement;
    "lx-native-text": LxNativeTextElement;
    "lx-native-button": LxNativeButtonElement;
    "lx-native-slider": LxNativeSliderElement;
  }
}
