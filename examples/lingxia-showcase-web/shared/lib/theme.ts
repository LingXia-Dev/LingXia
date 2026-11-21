/**
 * Resolved appearance for the parts of the UI that CSS cannot reach — canvas and
 * SVG drawn by JavaScript (the ECharts panels), for instance.
 *
 * The runtime stamps `data-theme="light|dark"` on `<html>` whenever the lxapp's
 * resolved appearance changes, and that attribute is authoritative: platform
 * media queries can lag an in-place switch. `prefers-color-scheme` is only the
 * fallback for the first paint and for browser previews.
 */

export type ResolvedTheme = "light" | "dark";

const DARK_QUERY = "(prefers-color-scheme: dark)";

function stampedTheme(): ResolvedTheme | null {
  const value = document.documentElement.getAttribute("data-theme");
  return value === "dark" || value === "light" ? value : null;
}

export function getResolvedTheme(): ResolvedTheme {
  if (typeof document === "undefined") return "light";
  const stamped = stampedTheme();
  if (stamped) return stamped;
  return window.matchMedia?.(DARK_QUERY).matches ? "dark" : "light";
}

/** Fires only on an actual branch change; returns an unsubscribe function. */
export function subscribeTheme(listener: (theme: ResolvedTheme) => void): () => void {
  if (typeof document === "undefined") return () => {};

  let current = getResolvedTheme();
  const notify = () => {
    const next = getResolvedTheme();
    if (next === current) return;
    current = next;
    listener(next);
  };

  const observer = new MutationObserver(notify);
  observer.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["data-theme"],
  });

  const media = window.matchMedia?.(DARK_QUERY);
  media?.addEventListener("change", notify);

  return () => {
    observer.disconnect();
    media?.removeEventListener("change", notify);
  };
}

export type ChartAxisColors = { label: string; line: string; split: string };

/**
 * Axis colors for JS-drawn charts, resolved from the live token variables so
 * theme/tokens.css stays the single source. Re-read after a theme change.
 */
export function chartAxisColors(): ChartAxisColors {
  const style = getComputedStyle(document.documentElement);
  const read = (name: string) => style.getPropertyValue(name).trim();
  return {
    label: read("--lx-text-secondary"),
    line: read("--lx-border"),
    split: read("--lx-border-subtle"),
  };
}
