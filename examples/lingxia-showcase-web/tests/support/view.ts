import type { ViewElement, ViewWindow } from '@lingxia/test';

// Types for the View probes this conformance suite runs with
// `t.app.view.eval(fn)`. The fixture's `ViewElement`/`ViewWindow` cover
// reading; a probe that has to change the DOM (arm a listener, stub a
// callback, click an element no locator may reach on purpose) casts to these
// inside its function. They are types only, so the function stays
// self-contained.

/** An element a probe writes to or drives directly. */
export interface ProbeElement extends ViewElement {
  readonly style: Record<string, string> & { setProperty(name: string, value: string): void };
  readonly dataset: Record<string, string | undefined>;
  readonly classList: { contains(name: string): boolean; add(...names: string[]): void; remove(...names: string[]): void };
  readonly parentElement: ProbeElement | null;
  readonly scrollHeight: number;
  readonly clientHeight: number;
  scrollTop: number;
  click(): void;
  focus(): void;
  blur(): void;
  remove(): void;
  scrollIntoView(options?: { block?: string; inline?: string; behavior?: string }): void;
  setAttribute(name: string, value: string): void;
  removeAttribute(name: string): void;
  appendChild<T>(child: T): T;
  dispatchEvent(event: unknown): boolean;
  addEventListener(type: string, listener: (event: unknown) => void, options?: unknown): void;
  removeEventListener(type: string, listener: (event: unknown) => void, options?: unknown): void;
}

/** A `<video>` element as a probe drives it. */
export interface ProbeVideo extends ProbeElement {
  readonly paused: boolean;
  readonly readyState: number;
  readonly currentTime: number;
  play(): Promise<void>;
  pause(): void;
}

/** The page `window` with the members probes write: markers and stubs. */
export interface ProbeWindow extends ViewWindow {
  [member: string]: unknown;
  scrollTo(x: number, y: number): void;
  requestAnimationFrame(callback: (time: number) => void): number;
  setTimeout(callback: () => void, ms?: number): number;
  addEventListener(type: string, listener: (event: unknown) => void, options?: unknown): void;
  removeEventListener(type: string, listener: (event: unknown) => void, options?: unknown): void;
  dispatchEvent(event: unknown): boolean;
  readonly Event: new (type: string, init?: { bubbles?: boolean; cancelable?: boolean }) => unknown;
  readonly CustomEvent: new (type: string, init?: { bubbles?: boolean; detail?: unknown }) => unknown;
}

/** The page `document` with the members probes write. */
export interface ProbeDocument {
  createElement(tag: string): ProbeElement;
  readonly body: ProbeElement;
  readonly head: ProbeElement;
  readonly readyState: string;
  addEventListener(type: string, listener: (event: unknown) => void, options?: unknown): void;
  removeEventListener(type: string, listener: (event: unknown) => void, options?: unknown): void;
  dispatchEvent(event: unknown): boolean;
}
