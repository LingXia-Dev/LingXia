import type {
  LinkTarget,
  NavigatorEdge,
  NavigatorFloatPosition,
  NavigatorInteraction,
  NavigatorPlacement,
  NavigatorSize,
  NavigatorWindowChrome,
} from "./navigator.js";
import type { LxMediaSwiperItem } from "./media_swiper.js";

export function appendDataAttrs(
  attrs: Record<string, unknown>,
  target: Record<string, string>
): void {
  for (const [key, value] of Object.entries(attrs)) {
    if (value === undefined || value === null) continue;
    if (!key.startsWith("data-")) continue;
    target[key] = String(value);
  }
}

export function appendPassthroughAttrs(
  attrs: Record<string, unknown>,
  target: Record<string, string>
): void {
  for (const [key, value] of Object.entries(attrs)) {
    if (value === undefined || value === null) continue;
    if (key in target) continue;
    if (key === "class" || key === "className" || key === "style" || key === "children" || key === "ref") {
      continue;
    }
    if (key.startsWith("on")) continue;
    if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
      target[key] = String(value);
    }
  }
}

export interface NavigatorNativeAttrOptions {
  to?: LinkTarget;
  replace?: boolean;
  reset?: boolean;
  as?: NavigatorPlacement;
  edge?: NavigatorEdge;
  position?: NavigatorFloatPosition;
  chrome?: NavigatorWindowChrome;
  size?: NavigatorSize;
  interaction?: NavigatorInteraction;
  hoverClass?: string;
  hoverStopPropagation?: boolean;
  hoverStartTime?: number;
  hoverStayTime?: number;
}

export function buildNavigatorNativeAttrs(
  options: NavigatorNativeAttrOptions,
  extraAttrs: Record<string, unknown> = {}
): Record<string, string> {
  const result: Record<string, string> = {
    "hover-class": options.hoverClass ?? "navigator-hover",
    "hover-stop-propagation": String(options.hoverStopPropagation ?? false),
    "hover-start-time": String(options.hoverStartTime ?? 20),
    "hover-stay-time": String(options.hoverStayTime ?? 70),
  };

  if (options.to) result.to = JSON.stringify(options.to);
  if (options.replace) result.replace = "";
  if (options.reset) result.reset = "";
  if (options.as) result.as = options.as;
  if (options.edge) result.edge = options.edge;
  if (options.position) result.position = options.position;
  if (options.chrome) result.chrome = options.chrome;
  if (options.size) result.size = JSON.stringify(options.size);
  if (options.interaction) result.interaction = JSON.stringify(options.interaction);

  appendPassthroughAttrs(extraAttrs, result);
  return result;
}

export const MEDIA_SWIPER_DOM_EVENT_MAP = {
  onChange: "change",
  onTransitionEnd: "transitionend",
  onEndReached: "endreached",
  onTap: "tap",
  onVideoEnded: "videoended",
  onError: "error",
} as const;

export interface MediaSwiperNativeAttrOptions {
  id?: string;
  items?: readonly LxMediaSwiperItem[];
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
  peek?: number | { previous?: number; next?: number };
}

export function buildMediaSwiperNativeAttrs(
  options: MediaSwiperNativeAttrOptions,
  extraAttrs: Record<string, unknown> = {}
): Record<string, string | number> {
  const result: Record<string, string | number> = {};
  const explicitId = typeof options.id === "string" ? options.id.trim() : "";

  if (explicitId.length > 0) result.id = explicitId;
  if (Array.isArray(options.items)) result.items = JSON.stringify(options.items);
  if (typeof options.index === "number") result.index = options.index;
  if (typeof options.initialIndex === "number") result["initial-index"] = options.initialIndex;
  if (options.loop) result.loop = "";
  if (options.autoplay) result.autoplay = "";
  if (typeof options.interval === "number") result.interval = options.interval;
  if (options.animation) result.animation = options.animation;
  if (typeof options.animationDuration === "number") result["animation-duration"] = options.animationDuration;
  if (options.direction) result.direction = options.direction;
  if (typeof options.contentRotate === "number") result["content-rotate"] = options.contentRotate;
  if (options.objectFit) result["object-fit"] = options.objectFit;
  if (options.controls) result.controls = "";
  if (options.muted === false) result.muted = "false";
  if (options.muted === true) result.muted = "";
  if (options.dots === true) result.dots = "";
  if (options.dots && typeof options.dots === "object") result.dots = JSON.stringify(options.dots);
  if (options.swipeEnabled === false) result["swipe-enabled"] = "false";
  if (typeof options.peek === "number" && options.peek > 0) {
    result.peek = options.peek;
  } else if (options.peek && typeof options.peek === "object") {
    const previous = Math.max(0, Number(options.peek.previous) || 0);
    const next = Math.max(0, Number(options.peek.next) || 0);
    if (previous > 0 || next > 0) {
      result.peek = JSON.stringify({ previous, next });
    }
  }

  const extraNativeAttrs: Record<string, string> = {};
  appendPassthroughAttrs(extraAttrs, extraNativeAttrs);
  return { ...result, ...extraNativeAttrs };
}
