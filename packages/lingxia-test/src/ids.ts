import { remapPosition } from "./host.js";

export interface StackFrame {
  file: string;
  line: number;
  column: number;
}

export function isAsciiTitle(title: string): boolean {
  for (let i = 0; i < title.length; i += 1) {
    if (title.charCodeAt(i) > 0x7f) return false;
  }
  return true;
}

export function slugTitle(title: string): string | undefined {
  if (!isAsciiTitle(title)) return undefined;
  const slug = title
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return slug.length > 0 ? slug : undefined;
}

export function fileStem(file: string): string {
  const normalized = file.replace(/\\/g, "/");
  const base = normalized.slice(normalized.lastIndexOf("/") + 1);
  return base.replace(/\.(test\.)?(ts|mts|js|mjs)$/i, "") || "spec";
}

/**
 * V8 writes `at name (file:line:col)`; JavaScriptCore (macOS/iOS) and ArkJS
 * write `name@file:line:col`. A report that only understands V8 shows
 * `unknown:0:0` for every location on those hosts, so parse both.
 */
export function parseFrames(stack: string | undefined): StackFrame[] {
  if (!stack) return [];
  const frames: StackFrame[] = [];
  for (const raw of stack.split("\n")) {
    const line = raw.trim();
    if (line.length === 0 || line.endsWith("[native code]")) continue;
    const match =
      line.match(/^at\s+.*\((.+):(\d+):(\d+)\)$/) ??
      line.match(/^at\s+(.+):(\d+):(\d+)$/) ??
      line.match(/^.*?@(.+):(\d+):(\d+)$/);
    if (!match) continue;
    const file = stripFileUrl(match[1]!);
    if (file.length === 0 || file === "native") continue;
    frames.push({ file, line: Number(match[2]), column: Number(match[3]) });
  }
  return frames;
}

const UNKNOWN: StackFrame = { file: "unknown", line: 0, column: 0 };

/** `@lingxia/test`'s own frames are never the interesting caller. */
function isFrameworkFrame(file: string): boolean {
  const normalized = file.replace(/\\/g, "/");
  return (
    /(^|\/)lingxia-test\/(dist|src)\//.test(normalized) ||
    normalized.includes("node_modules/@lingxia/test/") ||
    normalized.startsWith("node:")
  );
}

/**
 * The first authored frame, remapped through the bundle source map when the
 * CLI installed one. Bundled runs put the framework and the spec in one file,
 * so only the mapped path can tell them apart.
 */
export function resolveOrigin(frames: StackFrame[]): StackFrame {
  let firstMapped: StackFrame | undefined;
  for (const frame of frames) {
    const mapped = remapPosition(frame.file, frame.line, frame.column);
    if (!firstMapped) firstMapped = mapped;
    if (!isFrameworkFrame(mapped.file)) return mapped;
  }
  return firstMapped ?? frames[0] ?? UNKNOWN;
}

export function callerLocation(): StackFrame {
  return resolveOrigin(parseFrames(new Error().stack));
}

/** Raw frames for a location that can only be resolved once the map exists. */
export function captureFrames(): StackFrame[] {
  return parseFrames(new Error().stack);
}

export function stripFileUrl(path: string): string {
  if (path.startsWith("file://")) {
    const url = path.slice("file://".length);
    return /^\/[A-Za-z]:/.test(url) ? url.slice(1) : url;
  }
  return path;
}

export function displayLocation(file: string, line: number, column: number): string {
  const normalized = file.replace(/\\/g, "/");
  const parts = normalized.split("/");
  const shown = parts.length > 2 ? parts.slice(-3).join("/") : normalized;
  return `${shown}:${line}:${column}`;
}
