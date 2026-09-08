import type { AuthorNode } from "./types.js";

// Computed values catch unsupported styles from classes as well as inline CSS.
const unsupportedDefaults: Record<string, readonly string[]> = {
  transform: ["none"], translate: ["none"], rotate: ["none"], scale: ["none"],
  perspective: ["none"], clipPath: ["none"], maskImage: ["none"],
  boxShadow: ["none"], filter: ["none"], backdropFilter: ["none"],
  mixBlendMode: ["normal"], backgroundImage: ["none"],
  animationName: ["none"], transitionDuration: ["0s"],
  textDecorationLine: ["none"], fontStyle: ["normal"],
  letterSpacing: ["normal", "0px"], textShadow: ["none"],
};
const layoutProperties = new Set([
  "transform", "translate", "rotate", "scale", "perspective", "clipPath", "maskImage",
]);

export const OBSERVED_STYLE_FIELDS = [
  ...Object.keys(unsupportedDefaults), "display", "visibility", "opacity", "overflowX", "overflowY",
  "color", "backgroundColor", "accentColor", "fontSize", "fontWeight", "lineHeight", "textAlign", "direction",
  ...["Top", "Right", "Bottom", "Left"].flatMap(side => ["Width", "Style", "Color"].map(suffix => `border${side}${suffix}`)),
  "borderTopLeftRadius", "borderTopRightRadius", "borderBottomLeftRadius", "borderBottomRightRadius",
];

/** Native nodes are flat siblings, so each receives its cumulative alpha. */
export function effectiveOpacity(element: Element): number {
  let opacity = 1;
  for (let node: Element | null = element; node; node = node.parentElement) {
    opacity *= Number(node.ownerDocument.defaultView?.getComputedStyle(node).opacity ?? 1);
  }
  return opacity;
}

const colorContexts = new WeakMap<Document, CanvasRenderingContext2D>();
const colors = new Map<string, string>();

/** Canvas resolves browser-supported CSS colors into the hosts' sRGB wire format. */
export function normalizeNativeColor(document: Document, value: string): string | undefined {
  if (/^rgba?\([\d.,\s]+\)$/.test(value) || /^#[\da-f]{6}([\da-f]{2})?$/i.test(value)) return value;
  const cached = colors.get(value);
  if (cached) return cached;
  let context = colorContexts.get(document);
  if (!context) {
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = 1;
    context = canvas.getContext("2d", { willReadFrequently: true, colorSpace: "srgb" }) ?? undefined;
    if (!context) return undefined;
    colorContexts.set(document, context);
  }
  context.fillStyle = "#010203";
  context.fillStyle = value;
  const first = context.fillStyle;
  context.fillStyle = "#040506";
  context.fillStyle = value;
  if (context.fillStyle !== first) return undefined;
  context.clearRect(0, 0, 1, 1);
  context.fillRect(0, 0, 1, 1);
  const [r, g, b, a] = context.getImageData(0, 0, 1, 1).data;
  const result = `rgba(${r}, ${g}, ${b}, ${a / 255})`;
  if (colors.size >= 256) colors.clear();
  colors.set(value, result);
  return result;
}

export function collectNativeStyleIssues(element: Element): NonNullable<AuthorNode["styleIssues"]> {
  const view = element.ownerDocument?.defaultView;
  if (!view?.getComputedStyle) return [];
  const style = view.getComputedStyle(element);
  const issues: NonNullable<AuthorNode["styleIssues"]> = [];
  for (const property of ["color", "backgroundColor", "borderTopColor", "accentColor"] as const) {
    const value = style[property];
    if (value && value !== "auto" && !normalizeNativeColor(element.ownerDocument, value)) {
      issues.push({ property, value, layout: false });
    }
  }
  for (const [property, defaults] of Object.entries(unsupportedDefaults)) {
    const value = String(style[property as keyof CSSStyleDeclaration] ?? "").trim();
    if (!value || defaults.includes(value)) continue;
    if (property === "backgroundImage" && isVideoPoster(element, value)) continue;
    if (property === "transitionDuration" && value.split(",").every((duration) => parseFloat(duration) === 0)) continue;
    issues.push({ property, value, layout: layoutProperties.has(property) });
  }
  const corners = [style.borderTopLeftRadius, style.borderTopRightRadius,
    style.borderBottomRightRadius, style.borderBottomLeftRadius];
  if (corners.some((value) => value && (value !== corners[0] || !/^\d+(\.\d+)?px$/.test(value)))) {
    issues.push({ property: "borderRadius", value: corners.join(" / "), layout: false });
  }
  for (const suffix of ["Width", "Color", "Style"] as const) {
    const values = [style[`borderTop${suffix}`], style[`borderRight${suffix}`],
      style[`borderBottom${suffix}`], style[`borderLeft${suffix}`]];
    const hasBorder = [style.borderTopWidth, style.borderRightWidth,
      style.borderBottomWidth, style.borderLeftWidth].some((width) => parseFloat(width) > 0);
    if (hasBorder && (values.some((value) => value !== values[0]) ||
        (suffix === "Style" && values.some((value) => value !== "solid" && value !== "none")))) {
      issues.push({ property: `border${suffix}`, value: values.join(" / "), layout: false });
    }
  }
  return issues;
}

function isVideoPoster(element: Element, background: string): boolean {
  if (element.tagName.toLowerCase() !== "lx-video") return false;
  const poster = element.getAttribute("poster");
  const url = /^url\("(.*)"\)$/.exec(background)?.[1];
  if (!poster || !url) return false;
  // LxVideo owns this DOM placeholder; the native backend paints its poster prop.
  try {
    return new URL(url.replace(/\\(["\\])/g, "$1"), element.baseURI).href === new URL(poster, element.baseURI).href;
  } catch {
    return false;
  }
}
