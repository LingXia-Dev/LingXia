import type { NodeRef } from "./types.js";

export const VIDEO_COMMANDS = [
  "play",
  "pause",
  "stop",
  "seek",
  "enterFullscreen",
  "exitFullscreen",
  "setStreamSource",
] as const;

export type VideoCommandName = (typeof VIDEO_COMMANDS)[number];

export type VideoCommand =
  | { name: "play" }
  | { name: "pause" }
  | { name: "stop" }
  | { name: "seek"; seconds: number }
  | { name: "enterFullscreen" }
  | { name: "exitFullscreen" }
  | { name: "setStreamSource"; options: Record<string, unknown> };

export interface VideoCommandRequest {
  action: "video.command";
  owner: NodeRef;
  requestId: string;
  command: VideoCommand;
}

export interface VideoControlDescriptorBase {
  controlId: string;
  label: string;
  frame: { x: number; y: number; width: number; height: number };
  visible: boolean;
  disabled?: boolean;
}

export type VideoControlDescriptor =
  | (VideoControlDescriptorBase & {
      role: "button";
      pressed?: boolean;
      expanded?: boolean;
      actions: ReadonlyArray<"focus" | "blur" | "activate">;
    })
  | (VideoControlDescriptorBase & {
      role: "slider";
      value: number;
      min: number;
      max: number;
      actions: ReadonlyArray<"focus" | "blur" | "increment" | "decrement" | "setValue">;
    });

export interface VideoControlsSemanticSnapshot {
  action: "video.controlsSemanticSnapshot";
  owner: NodeRef;
  revision: number;
  controls: readonly VideoControlDescriptor[];
}

export function buildVideoCommandRequest(
  owner: NodeRef,
  command: VideoCommand,
  requestId: string
): VideoCommandRequest {
  return { action: "video.command", owner, requestId, command };
}

export function videoCommandUrls(command: VideoCommand): string[] {
  if (command.name !== "setStreamSource") return [];
  const options = command.options ?? {};
  const urls: string[] = [];
  for (const key of ["url", "src", "uri"]) {
    const value = options[key];
    if (typeof value === "string" && value) urls.push(value);
  }
  return urls;
}

export function collectVideoResourceUrls(props: Record<string, unknown>): string[] {
  const urls: string[] = [];
  for (const key of ["src", "poster"]) {
    const value = props[key];
    if (typeof value === "string" && value) urls.push(value);
  }
  const watermark = props.watermark as { resource?: { url?: string } } | undefined;
  if (watermark?.resource && typeof watermark.resource === "object") {
    const url = (watermark.resource as { url?: string }).url;
    if (typeof url === "string" && url) urls.push(url);
  }
  const qualities = props.qualities;
  if (Array.isArray(qualities)) {
    for (const item of qualities) {
      if (item && typeof item === "object" && typeof (item as { url?: string }).url === "string") {
        urls.push((item as { url: string }).url);
      }
    }
  }
  return urls;
}

export function validateControlsSnapshot(
  snapshot: VideoControlsSemanticSnapshot,
  lastRevision: number
): { ok: true } | { ok: false; message: string } {
  if (snapshot.revision <= lastRevision) {
    return { ok: false, message: "controls snapshot revision must increase" };
  }
  const seen = new Set<string>();
  for (const control of snapshot.controls) {
    if (!control.controlId || seen.has(control.controlId)) {
      return { ok: false, message: "controlId must be unique and non-empty" };
    }
    seen.add(control.controlId);
    if (control.role === "slider") {
      if (![control.min, control.max, control.value].every(Number.isFinite)) {
        return { ok: false, message: "slider descriptor values must be finite" };
      }
      if (control.min > control.value || control.value > control.max) {
        return { ok: false, message: "slider descriptor value is out of range" };
      }
    }
  }
  return { ok: true };
}
