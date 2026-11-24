import * as React from "react";
import { getHost, holdBackHandler, subscribeHost } from "@lingxia/bridge";

export type SizeClass = "compact" | "regular";

export interface Edges {
  readonly top: number;
  readonly right: number;
  readonly bottom: number;
  readonly left: number;
  readonly inlineStart: number;
  readonly inlineEnd: number;
}

export interface HingeRect {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly orientation: "vertical" | "horizontal";
  readonly posture: "flat" | "half-opened";
}

export interface LxViewEnv {
  readonly sizeClass: SizeClass;
  readonly viewport: Readonly<{ width: number; height: number }>;
  readonly insets: Readonly<Edges>;
  readonly safeArea: Readonly<Edges>;
  readonly hinge: Readonly<HingeRect> | null;
  readonly keyboard: Readonly<{ height: number }>;
  readonly pointer: "fine" | "coarse";
  readonly direction: "ltr" | "rtl";
  readonly theme: "light" | "dark";
  readonly highContrast: boolean;
  readonly reducedMotion: boolean;
  readonly animatorDurationScale: number;
  readonly language: string;
  readonly hourCycle: 12 | 24;
  readonly platform: "apple" | "android" | "harmony" | "windows" | "web";
}

const SizeClassContext = React.createContext<SizeClass | null>(null);

/**
 * Overrides `useLxViewEnvironment().sizeClass` and `[data-size-class]` CSS
 * matching for its subtree, such as a pane narrower than the window.
 */
export function SizeClassScope({
  value,
  children,
}: {
  value: SizeClass;
  children?: React.ReactNode;
}) {
  return (
    <SizeClassContext.Provider value={value}>
      <div data-size-class={value} style={{ display: "contents" }}>
        {children}
      </div>
    </SizeClassContext.Provider>
  );
}

function normalizePlatform(os: string | undefined): LxViewEnv["platform"] {
  const lower = (os ?? "").toLowerCase();
  if (lower.includes("ios") || lower.includes("mac") || lower.includes("apple")) return "apple";
  if (lower.includes("android")) return "android";
  if (lower.includes("harmony") || lower.includes("ohos")) return "harmony";
  if (lower.includes("win")) return "windows";
  return "web";
}

function resolveHysteresisSizeClass(prev: SizeClass | null, width: number): SizeClass {
  const raw: SizeClass = width < 600 ? "compact" : "regular";
  if (prev === null || prev === raw) return raw;
  return Math.abs(width - 600) < 24 ? prev : raw;
}

interface PageChromeWindow {
  lxPageChrome?: {
    layout?: {
      topInset?: number;
      bottomInset?: number;
      capsuleInlineEndInset?: number;
    };
  };
}

function readHourCycle(locale: string): 12 | 24 {
  try {
    return new Intl.DateTimeFormat(locale, { hour: "numeric" }).resolvedOptions().hour12 ? 12 : 24;
  } catch {
    return 24;
  }
}

/** The part of the layout viewport the on-screen keyboard covers. */
function readKeyboardHeight(): number {
  if (typeof window === "undefined" || !window.visualViewport) return 0;
  const { height, offsetTop } = window.visualViewport;
  return Math.max(0, Math.round(window.innerHeight - height - offsetTop));
}

function readSnapshot(scoped: SizeClass | null, prevHostClass: SizeClass | null): LxViewEnv {
  const host = getHost();
  const width = typeof window !== "undefined" ? window.innerWidth : 0;
  const height = typeof window !== "undefined" ? window.innerHeight : 0;
  const hostSizeClass: SizeClass =
    host.sizeClass === "compact" || host.sizeClass === "regular"
      ? host.sizeClass
      : resolveHysteresisSizeClass(prevHostClass, width);
  const direction: "ltr" | "rtl" =
    typeof document !== "undefined" && document.documentElement.dir === "rtl" ? "rtl" : "ltr";
  const chrome =
    typeof window !== "undefined" ? (window as unknown as PageChromeWindow).lxPageChrome?.layout : undefined;
  const top = chrome?.topInset ?? 0;
  const bottom = chrome?.bottomInset ?? 0;
  const inlineEnd = chrome?.capsuleInlineEndInset ?? 0;
  const left = direction === "rtl" ? inlineEnd : 0;
  const right = direction === "rtl" ? 0 : inlineEnd;
  const insets: Edges = Object.freeze({
    top,
    right,
    bottom,
    left,
    inlineStart: 0,
    inlineEnd,
  });
  const safeArea: Edges = Object.freeze({
    top,
    right: 0,
    bottom,
    left: 0,
    inlineStart: 0,
    inlineEnd: 0,
  });
  const dark =
    typeof window !== "undefined" &&
    (document.documentElement?.getAttribute("data-theme") === "dark" ||
      window.matchMedia?.("(prefers-color-scheme: dark)").matches);
  const reducedMotion =
    typeof window !== "undefined" &&
    window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
  const highContrast =
    typeof window !== "undefined" && window.matchMedia?.("(prefers-contrast: more)").matches;
  const pointer: "fine" | "coarse" =
    typeof window !== "undefined" && window.matchMedia?.("(pointer: fine)").matches
      ? "fine"
      : "coarse";

  return Object.freeze({
    sizeClass: scoped ?? hostSizeClass,
    viewport: Object.freeze({ width, height }),
    insets,
    safeArea,
    // WebView pages get no fold geometry from the host; only GPU pages report a hinge.
    hinge: null,
    keyboard: Object.freeze({ height: readKeyboardHeight() }),
    pointer,
    direction,
    theme: dark ? "dark" : "light",
    highContrast: Boolean(highContrast),
    reducedMotion: Boolean(reducedMotion),
    animatorDurationScale: reducedMotion ? 0 : 1,
    language: host.language || "en",
    hourCycle: readHourCycle(host.language || "en"),
    platform: normalizePlatform(host.os),
  });
}

function shallowFieldEqual(a: unknown, b: unknown): boolean {
  if (Object.is(a, b)) return true;
  if (!a || !b || typeof a !== "object" || typeof b !== "object") return false;
  const aKeys = Object.keys(a as Record<string, unknown>);
  const bKeys = Object.keys(b as Record<string, unknown>);
  if (aKeys.length !== bKeys.length) return false;
  return aKeys.every((k) =>
    Object.is((a as Record<string, unknown>)[k], (b as Record<string, unknown>)[k]),
  );
}

/**
 * Isomorphic with GPU `view.env`: returns a field-tracked Proxy so components
 * that only read `sizeClass` perform 0 re-renders while dragging a window.
 */
export function useLxViewEnvironment(): LxViewEnv {
  const scoped = React.useContext(SizeClassContext);
  const readKeysRef = React.useRef<Set<keyof LxViewEnv>>(new Set());
  const prevHostClassRef = React.useRef<SizeClass | null>(null);
  const snapshotRef = React.useRef<LxViewEnv>(readSnapshot(scoped, prevHostClassRef.current));
  const [, forceUpdate] = React.useReducer((n: number) => n + 1, 0);

  // Reset read set each render so unused keys stop triggering re-renders.
  readKeysRef.current = new Set();
  snapshotRef.current = readSnapshot(scoped, prevHostClassRef.current);
  if (!scoped) prevHostClassRef.current = snapshotRef.current.sizeClass;

  React.useEffect(() => {
    const check = () => {
      const next = readSnapshot(scoped, prevHostClassRef.current);
      if (!scoped) prevHostClassRef.current = next.sizeClass;
      const prev = snapshotRef.current;
      let changed = false;
      for (const key of readKeysRef.current) {
        if (!shallowFieldEqual(prev[key], next[key])) {
          changed = true;
          break;
        }
      }
      if (changed) {
        snapshotRef.current = next;
        forceUpdate();
      }
    };
    const unsubHost = subscribeHost(check);
    if (typeof window !== "undefined") {
      window.addEventListener("resize", check, { passive: true });
      window.addEventListener("lxpagechromechange", check);
      window.visualViewport?.addEventListener("resize", check);
    }
    return () => {
      unsubHost();
      if (typeof window !== "undefined") {
        window.removeEventListener("resize", check);
        window.removeEventListener("lxpagechromechange", check);
        window.visualViewport?.removeEventListener("resize", check);
      }
    };
  }, [scoped]);

  return new Proxy(snapshotRef.current, {
    get(target, prop, receiver) {
      if (typeof prop === "string") {
        readKeysRef.current.add(prop as keyof LxViewEnv);
      }
      return Reflect.get(target, prop, receiver);
    },
  });
}

/** LIFO back-navigation handler (intercepts Back only, never Home). */
export function useBackHandler(enabled: boolean, onBack: () => void): void {
  const latest = React.useRef(onBack);
  React.useEffect(() => {
    latest.current = onBack;
  });
  React.useEffect(() => {
    if (!enabled) return undefined;
    return holdBackHandler(() => latest.current());
  }, [enabled]);
}
