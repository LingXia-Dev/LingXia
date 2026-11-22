import * as React from "react";
import { getHost, holdLeaveGuard, subscribeHost } from "@lingxia/bridge";

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
 * matching for its subtree (used inside `<ListDetail>` and `<Split>` panes).
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
    return holdLeaveGuard((request) => {
      if (request.reason !== "back") return true;
      latest.current();
      return false;
    });
  }, [enabled]);
}

interface ListDetailContextValue {
  isCompact: boolean;
  hasSelection: boolean;
  clearSelection: () => void;
  listLabel: string;
  listWidth: number;
  hingeGapWidth: number;
}

const ListDetailCtx = React.createContext<ListDetailContextValue | null>(null);

export interface ListDetailProps {
  value?: string | null;
  onChange?: (value: string | null) => void;
  listLabel: string;
  listWidth?: number;
  children?: React.ReactNode;
  className?: string;
}

function ListDetailList({
  children,
  className,
}: {
  children?: React.ReactNode;
  className?: string;
}) {
  const ctx = React.useContext(ListDetailCtx);
  if (!ctx) throw new Error("ListDetail.List must be rendered inside <ListDetail>");
  if (ctx.isCompact) {
    return (
      <div
        role="group"
        aria-label={ctx.listLabel}
        className={className}
        style={{
          display: ctx.hasSelection ? "none" : "flex",
          flexDirection: "column",
          flex: 1,
          minHeight: 0,
        }}
      >
        {children}
      </div>
    );
  }
  return (
    <SizeClassScope value="compact">
      <div
        role="group"
        aria-label={ctx.listLabel}
        className={className}
        style={{ width: ctx.listWidth, flexShrink: 0, display: "flex", flexDirection: "column" }}
      >
        {children}
      </div>
    </SizeClassScope>
  );
}

function ListDetailDetail({
  title,
  placeholder,
  children,
  className,
}: {
  title?: string;
  placeholder?: React.ReactNode;
  children?: React.ReactNode;
  className?: string;
}) {
  const ctx = React.useContext(ListDetailCtx);
  if (!ctx) throw new Error("ListDetail.Detail must be rendered inside <ListDetail>");
  if (ctx.isCompact) {
    if (!ctx.hasSelection) return null;
    return (
      <div
        role="group"
        aria-label={title ?? ctx.listLabel}
        className={className}
        style={{ flex: 1, minHeight: 0, display: "flex", flexDirection: "column" }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "8px 12px" }}>
          <button type="button" onClick={ctx.clearSelection}>
            <span aria-hidden="true">‹ </span>
            {ctx.listLabel}
          </button>
          {title && <strong>{title}</strong>}
        </div>
        {children}
      </div>
    );
  }
  return (
    <SizeClassScope value="compact">
      <div
        role="group"
        aria-label={title ?? ctx.listLabel}
        className={className}
        style={{
          flex: 1,
          minWidth: 0,
          display: "flex",
          flexDirection: "column",
          marginInlineStart: ctx.hingeGapWidth > 0 ? ctx.hingeGapWidth : undefined,
        }}
      >
        {ctx.hasSelection ? children : (placeholder ?? null)}
      </div>
    </SizeClassScope>
  );
}

function ListDetailRoot({
  value,
  onChange,
  listLabel,
  listWidth = 360,
  children,
  className,
}: ListDetailProps) {
  const env = useLxViewEnvironment();
  const isCompact = env.sizeClass === "compact";
  const hasSelection = value !== null && value !== undefined && value !== "";
  const clearSelection = React.useCallback(() => onChange?.(null), [onChange]);
  useBackHandler(isCompact && hasSelection, clearSelection);

  const verticalHinge =
    !isCompact && env.hinge && env.hinge.orientation === "vertical" ? env.hinge : null;
  const hingeGapWidth = verticalHinge ? Math.max(0, verticalHinge.width) : 0;
  const viewportWidth = env.viewport.width;
  const resolvedListWidth = verticalHinge
    ? env.direction === "rtl" && viewportWidth > 0
      ? Math.max(200, viewportWidth - verticalHinge.x - hingeGapWidth)
      : Math.max(200, verticalHinge.x)
    : listWidth;

  return (
    <ListDetailCtx.Provider
      value={{
        isCompact,
        hasSelection,
        clearSelection,
        listLabel,
        listWidth: resolvedListWidth,
        hingeGapWidth,
      }}
    >
      <div
        className={className}
        style={{
          display: "flex",
          flexDirection: isCompact ? "column" : "row",
          flex: 1,
          minHeight: 0,
        }}
      >
        {children}
      </div>
    </ListDetailCtx.Provider>
  );
}

export const ListDetail = Object.assign(ListDetailRoot, {
  List: ListDetailList,
  Detail: ListDetailDetail,
});

export interface SplitProps {
  children?: React.ReactNode;
  aside: React.ReactNode;
  asideWidth?: number;
  asideTitle?: string;
  compact?: "stack" | "sheet";
  isOpen?: boolean;
  open?: boolean;
  onOpenChange?: (isOpen: boolean) => void;
  /** The compact sheet's close button label; iOS has no system Back to dismiss it. @default "Close" */
  closeLabel?: string;
  className?: string;
}

export function Split({
  children,
  aside,
  asideWidth = 320,
  asideTitle = "Details",
  compact = "stack",
  isOpen,
  open,
  onOpenChange,
  closeLabel = "Close",
  className,
}: SplitProps) {
  const env = useLxViewEnvironment();
  const isCompact = env.sizeClass === "compact";
  const hinge = env.hinge;
  const viewportWidth = env.viewport.width;
  const sheetOpen = Boolean(isOpen ?? open);
  const closeSheet = React.useCallback(() => onOpenChange?.(false), [onOpenChange]);
  useBackHandler(isCompact && compact === "sheet" && sheetOpen, closeSheet);

  if (hinge && hinge.orientation === "horizontal" && hinge.posture === "half-opened") {
    const upperHeight = Math.max(120, hinge.y);
    const gapHeight = Math.max(0, hinge.height);
    const leafScope: SizeClass = viewportWidth >= 600 ? "regular" : "compact";
    return (
      <div
        className={className}
        style={{ display: "flex", flexDirection: "column", flex: 1, minHeight: 0 }}
      >
        <SizeClassScope value={leafScope}>
          <div style={{ height: upperHeight, flexShrink: 0, minWidth: 0 }}>{children}</div>
        </SizeClassScope>
        {gapHeight > 0 && <div aria-hidden="true" style={{ height: gapHeight, flexShrink: 0 }} />}
        <SizeClassScope value={leafScope}>
          <div style={{ flex: 1, minHeight: 0 }}>{aside}</div>
        </SizeClassScope>
      </div>
    );
  }

  if (isCompact) {
    return (
      <div
        className={className}
        style={{ display: "flex", flexDirection: "column", flex: 1, minHeight: 0 }}
      >
        <div style={{ flex: 1, minHeight: 0 }}>{children}</div>
        {compact === "stack" && <div style={{ flexShrink: 0 }}>{aside}</div>}
        {compact === "sheet" && sheetOpen && (
          <div role="dialog" aria-label={asideTitle} style={{ flexShrink: 0 }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "8px 12px" }}>
              <strong style={{ flex: 1, minWidth: 0 }}>{asideTitle}</strong>
              <button type="button" onClick={closeSheet}>
                {closeLabel}
              </button>
            </div>
            {aside}
          </div>
        )}
      </div>
    );
  }

  const verticalHinge = hinge && hinge.orientation === "vertical" ? hinge : null;
  const hingeGapWidth = verticalHinge ? Math.max(0, verticalHinge.width) : 0;
  const resolvedAsideWidth =
    verticalHinge && viewportWidth > 0
      ? env.direction === "rtl"
        ? Math.max(240, verticalHinge.x)
        : Math.max(240, viewportWidth - verticalHinge.x - hingeGapWidth)
      : asideWidth;
  const primaryWidth = viewportWidth > 0 ? viewportWidth - resolvedAsideWidth - hingeGapWidth : 0;
  const primaryScope: SizeClass = primaryWidth >= 600 ? "regular" : "compact";
  const asideScope: SizeClass = resolvedAsideWidth >= 600 ? "regular" : "compact";

  return (
    <div
      className={className}
      style={{ display: "flex", flexDirection: "row", flex: 1, minHeight: 0 }}
    >
      <SizeClassScope value={primaryScope}>
        <div style={{ flex: 1, minWidth: 0 }}>{children}</div>
      </SizeClassScope>
      {hingeGapWidth > 0 && <div aria-hidden="true" style={{ width: hingeGapWidth, flexShrink: 0 }} />}
      <SizeClassScope value={asideScope}>
        <div style={{ width: resolvedAsideWidth, flexShrink: 0 }}>{aside}</div>
      </SizeClassScope>
    </div>
  );
}

export interface TopBarProps {
  leading?: React.ReactNode;
  title?: string;
  children?: React.ReactNode;
  trailing?: React.ReactNode;
  className?: string;
}

export function TopBar({ leading, title, children, trailing, className }: TopBarProps) {
  const env = useLxViewEnvironment();
  const isCompact = env.sizeClass === "compact";
  const inlineStart = env.insets.inlineStart;
  const inlineEnd = env.insets.inlineEnd;
  return (
    <header
      className={className}
      style={{
        display: "flex",
        flexDirection: "row",
        alignItems: "center",
        gap: 8,
        minHeight: 44,
        paddingInlineStart: isCompact && inlineStart > 0 ? inlineStart + 12 : 12,
        paddingInlineEnd: isCompact && inlineEnd > 0 ? inlineEnd + 12 : 12,
      }}
    >
      {leading !== undefined && <div style={{ flexShrink: 0 }}>{leading}</div>}
      <div
        style={{
          flex: 1,
          minWidth: 0,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
        }}
      >
        {children !== undefined ? children : title !== undefined ? <strong>{title}</strong> : null}
      </div>
      {trailing !== undefined && <div style={{ flexShrink: 0 }}>{trailing}</div>}
    </header>
  );
}
