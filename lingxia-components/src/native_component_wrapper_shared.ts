import type { NavigatorOpenType, NavigatorTarget } from "./navigator.js";
import type { LxVideoQuality } from "./video.js";

export function normalizeBindingAttrName(key: string): string {
  return key.replace(/[^a-zA-Z0-9]/g, "").toLowerCase();
}

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

export function appendBindingAndDatasetAttrs(
  attrs: Record<string, unknown>,
  target: Record<string, string>
): void {
  appendDataAttrs(attrs, target);
  for (const [key, value] of Object.entries(attrs)) {
    if ((key.startsWith("bind") || key.startsWith("catch")) && typeof value === "string") {
      target[normalizeBindingAttrName(key)] = value;
    }
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
    if ((key.startsWith("bind") || key.startsWith("catch")) && typeof value === "string") {
      target[normalizeBindingAttrName(key)] = value;
      continue;
    }
    if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
      target[key] = String(value);
    }
  }
}

export interface NavigatorNativeAttrOptions {
  url?: string;
  openType?: NavigatorOpenType;
  target?: NavigatorTarget;
  delta?: number;
  appId?: string;
  path?: string;
  phoneNumber?: string;
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
    "open-type": options.openType ?? "navigate",
    delta: String(options.delta ?? 1),
    "hover-class": options.hoverClass ?? "navigator-hover",
    "hover-stop-propagation": String(options.hoverStopPropagation ?? false),
    "hover-start-time": String(options.hoverStartTime ?? 20),
    "hover-stay-time": String(options.hoverStayTime ?? 70),
  };

  if (options.url) result.url = options.url;
  if (options.target) result.target = options.target;
  if (options.appId) result["app-id"] = options.appId;
  if (options.path) result.path = options.path;
  if (options.phoneNumber) result["phone-number"] = options.phoneNumber;

  appendPassthroughAttrs(extraAttrs, result);
  return result;
}

export type PickerColumns = string[][] | [string[], Record<string, string[]>];
export type PickerValue = string | string[] | undefined;
export type PickerFields = "year" | "month" | "day" | "range";

export interface PickerNativeAttrOptions {
  id?: string;
  columns?: PickerColumns;
  mode?: "date" | "time";
  start?: string;
  end?: string;
  fields?: PickerFields;
  modelValue?: string | string[];
  value?: string | string[];
  cancelText?: string;
  cancelTextColor?: string;
  cancelButtonColor?: string;
  confirmText?: string;
  confirmTextColor?: string;
  confirmButtonColor?: string;
  bindChange?: string;
  bindScroll?: string;
  catchChange?: string;
  catchScroll?: string;
}

export function isPickerDateMode(mode?: "date" | "time"): boolean {
  return mode === "date" || mode === "time";
}

export function isPickerCascading(columns?: PickerColumns): columns is [string[], Record<string, string[]>] {
  return !!columns && columns.length === 2 && typeof columns[1] === "object" && !Array.isArray(columns[1]);
}

export function isPickerSingle(columns?: PickerColumns): boolean {
  return !!columns && columns.length === 1;
}

export function getPickerIndexFromValue(
  columns: PickerColumns | undefined,
  value: PickerValue
): number | number[] {
  if (!columns) return 0;
  if (isPickerSingle(columns)) {
    if (typeof value !== "string") return 0;
    const idx = columns[0].indexOf(value);
    return idx >= 0 ? idx : 0;
  }
  if (!Array.isArray(value)) {
    return Array.from({ length: columns.length }, () => 0);
  }
  if (isPickerCascading(columns)) {
    const [keys, map] = columns;
    const keyIdx = Math.max(0, keys.indexOf(value[0]));
    const valIdx = Math.max(0, map[keys[keyIdx]]?.indexOf(value[1]) ?? 0);
    return [keyIdx, valIdx];
  }
  const indexes = value.map((item, index) => Math.max(0, columns[index]?.indexOf(item) ?? 0));
  while (indexes.length < columns.length) indexes.push(0);
  return indexes;
}

export function getPickerValueFromIndex(
  columns: PickerColumns | undefined,
  index: number | number[]
): string | string[] {
  if (!columns) return "";
  if (typeof index === "number") {
    return columns[0]?.[index] ?? "";
  }
  if (isPickerCascading(columns)) {
    const [keys, map] = columns;
    const key = keys[index[0]] ?? "";
    return [key, map[key]?.[index[1]] ?? ""];
  }
  return index.map((item, column) => columns[column]?.[item] ?? "");
}

export function getPickerDisplayText(value: PickerValue, fields?: PickerFields): string {
  if (!value) return "";
  if (fields === "range" && Array.isArray(value)) {
    return `${value[0]} ~ ${value[1]}`;
  }
  return typeof value === "string" ? value : value.join(" - ");
}

export function buildPickerNativeAttrs(
  options: PickerNativeAttrOptions,
  extraAttrs: Record<string, unknown> = {}
): Record<string, string> {
  const result: Record<string, string> = {};
  const explicitId = typeof options.id === "string" ? options.id.trim() : "";
  const nextValue = options.modelValue ?? options.value;

  if (explicitId.length > 0) result.id = explicitId;

  if (isPickerDateMode(options.mode)) {
    result.mode = options.mode!;
    if (options.fields) result.fields = options.fields;
    if (nextValue) {
      result.value = typeof nextValue === "string" ? nextValue : JSON.stringify(nextValue);
    }
    if (options.start) result.start = options.start;
    if (options.end) result.end = options.end;
  } else {
    result.mode = isPickerCascading(options.columns)
      ? "cascading"
      : isPickerSingle(options.columns)
        ? "selector"
        : "multiSelector";
    result.columns = JSON.stringify(options.columns ?? []);
    result["default-index"] = JSON.stringify(getPickerIndexFromValue(options.columns, nextValue));
  }

  if (options.cancelText) result["cancel-text"] = options.cancelText;
  if (options.cancelTextColor) result["cancel-text-color"] = options.cancelTextColor;
  if (options.cancelButtonColor) result["cancel-button-color"] = options.cancelButtonColor;
  if (options.confirmText) result["confirm-text"] = options.confirmText;
  if (options.confirmTextColor) result["confirm-text-color"] = options.confirmTextColor;
  if (options.confirmButtonColor) result["confirm-button-color"] = options.confirmButtonColor;
  if (options.bindChange) result.bindchange = options.bindChange;
  if (options.bindScroll) result.bindscroll = options.bindScroll;
  if (options.catchChange) result.catchchange = options.catchChange;
  if (options.catchScroll) result.catchscroll = options.catchScroll;

  appendPassthroughAttrs(extraAttrs, result);
  return result;
}

export const VIDEO_DOM_EVENT_MAP = {
  onPlayRequest: "playrequest",
  onPlay: "play",
  onPlaying: "playing",
  onPause: "pause",
  onStop: "stop",
  onEnded: "ended",
  onTimeUpdate: "timeupdate",
  onError: "error",
  onLoadedMetadata: "loadedmetadata",
  onFullscreenChange: "fullscreenchange",
  onWaiting: "waiting",
  onQualityChange: "qualitychange",
  onRateChange: "ratechange",
} as const;

export interface VideoNativeAttrOptions {
  id?: string;
  src?: string;
  poster?: string;
  autoplay?: boolean;
  loop?: boolean;
  muted?: boolean;
  controls?: boolean;
  progressBar?: boolean;
  live?: boolean;
  volume?: string | number;
  qualities?: LxVideoQuality[];
  playbackRates?: number[];
  bindPlayRequest?: string;
  bindPlay?: string;
  bindPlaying?: string;
  bindPause?: string;
  bindStop?: string;
  bindEnded?: string;
  bindTimeUpdate?: string;
  bindError?: string;
  bindLoadedMetadata?: string;
  bindFullscreenChange?: string;
  bindWaiting?: string;
  bindQualityChange?: string;
  bindRateChange?: string;
  catchPlayRequest?: string;
  catchPlay?: string;
  catchPlaying?: string;
  catchPause?: string;
  catchStop?: string;
  catchEnded?: string;
  catchTimeUpdate?: string;
  catchError?: string;
  catchLoadedMetadata?: string;
  catchFullscreenChange?: string;
  catchWaiting?: string;
  catchQualityChange?: string;
  catchRateChange?: string;
}

export function buildVideoNativeAttrs(
  options: VideoNativeAttrOptions,
  extraAttrs: Record<string, unknown> = {}
): Record<string, string | number> {
  const result: Record<string, string | number> = {};
  const explicitId = typeof options.id === "string" ? options.id.trim() : "";

  if (explicitId.length > 0) result.id = explicitId;
  if (options.src) result.src = options.src;
  if (options.poster) result.poster = options.poster;
  if (options.autoplay) result.autoplay = "";
  if (options.loop) result.loop = "";
  if (options.muted) result.muted = "";
  if (options.controls) result.controls = "";
  if (options.progressBar === false) result["progress-bar"] = "false";
  if (options.live) result.live = "";
  if (options.volume !== undefined) result.volume = options.volume;
  if (options.qualities?.length) result.qualities = JSON.stringify(options.qualities);
  if (options.playbackRates?.length) result["playback-rates"] = JSON.stringify(options.playbackRates);

  if (options.bindPlayRequest) result.bindplayrequest = options.bindPlayRequest;
  if (options.bindPlay) result.bindplay = options.bindPlay;
  if (options.bindPlaying) result.bindplaying = options.bindPlaying;
  if (options.bindPause) result.bindpause = options.bindPause;
  if (options.bindStop) result.bindstop = options.bindStop;
  if (options.bindEnded) result.bindended = options.bindEnded;
  if (options.bindTimeUpdate) result.bindtimeupdate = options.bindTimeUpdate;
  if (options.bindError) result.binderror = options.bindError;
  if (options.bindLoadedMetadata) result.bindloadedmetadata = options.bindLoadedMetadata;
  if (options.bindFullscreenChange) result.bindfullscreenchange = options.bindFullscreenChange;
  if (options.bindWaiting) result.bindwaiting = options.bindWaiting;
  if (options.bindQualityChange) result.bindqualitychange = options.bindQualityChange;
  if (options.bindRateChange) result.bindratechange = options.bindRateChange;
  if (options.catchPlayRequest) result.catchplayrequest = options.catchPlayRequest;
  if (options.catchPlay) result.catchplay = options.catchPlay;
  if (options.catchPlaying) result.catchplaying = options.catchPlaying;
  if (options.catchPause) result.catchpause = options.catchPause;
  if (options.catchStop) result.catchstop = options.catchStop;
  if (options.catchEnded) result.catchended = options.catchEnded;
  if (options.catchTimeUpdate) result.catchtimeupdate = options.catchTimeUpdate;
  if (options.catchError) result.catcherror = options.catchError;
  if (options.catchLoadedMetadata) result.catchloadedmetadata = options.catchLoadedMetadata;
  if (options.catchFullscreenChange) result.catchfullscreenchange = options.catchFullscreenChange;
  if (options.catchWaiting) result.catchwaiting = options.catchWaiting;
  if (options.catchQualityChange) result.catchqualitychange = options.catchQualityChange;
  if (options.catchRateChange) result.catchratechange = options.catchRateChange;

  const extraNativeAttrs: Record<string, string> = {};
  appendPassthroughAttrs(extraAttrs, extraNativeAttrs);
  return { ...result, ...extraNativeAttrs };
}
