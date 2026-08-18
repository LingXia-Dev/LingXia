import { bytesToBase64, utf8ToBase64 } from "./format.js";
import { PACKAGE_NAME, VERSION } from "./version.js";
import type { AutomationHost } from "./types.js";

export interface ResolvedHost {
  args: Record<string, string>;
  attach(
    name: string,
    artifact: { mimeType: string; base64: string },
  ): Promise<void>;
  emit(event: Record<string, unknown>): Promise<void>;
  logs(): Promise<string | undefined>;
}

function asArgs(value: unknown): Record<string, string> {
  if (!value || typeof value !== "object") return {};
  const out: Record<string, string> = {};
  for (const [key, item] of Object.entries(value as Record<string, unknown>)) {
    if (typeof item === "string") out[key] = item;
    else if (item != null) out[key] = String(item);
  }
  return out;
}

export function resolveHost(): ResolvedHost {
  const automation = globalThis.__LINGXIA_AUTOMATION_HOST__;
  const rong = globalThis.__RONG_TEST_HOST__;
  const raw: AutomationHost | undefined = automation ?? rong;
  return {
    args: asArgs(raw?.args),
    async attach(name, artifact) {
      if (!raw?.attach) return;
      await raw.attach(name, artifact);
    },
    async emit(event) {
      const emit = automation?.emit ?? rong?.report ?? raw?.emit ?? raw?.report;
      if (!emit) return;
      await emit(event);
    },
    async logs() {
      if (!raw?.logs) return undefined;
      const value = await raw.logs();
      if (Array.isArray(value)) return value.join("\n");
      if (typeof value === "string" && value.length > 0) return value;
      return undefined;
    },
  };
}

export function warnVersionSkew(): void {
  const cli = globalThis.__LINGXIA_CLI_VERSION__;
  if (!cli || cli === VERSION) return;
  console.warn(
    `${PACKAGE_NAME}@${VERSION} does not match lxdev ${cli}; use matching package and CLI versions.`,
  );
}

export async function attachText(
  host: ResolvedHost,
  name: string,
  text: string,
  mimeType: string,
): Promise<void> {
  await host.attach(name, { mimeType, base64: utf8ToBase64(text) });
}

export async function attachBytes(
  host: ResolvedHost,
  name: string,
  bytes: Uint8Array,
  mimeType: string,
): Promise<void> {
  await host.attach(name, { mimeType, base64: bytesToBase64(bytes) });
}

export function encodeAttachPayload(data: unknown): { mimeType: string; base64: string } {
  if (data && typeof data === "object") {
    const record = data as Record<string, unknown>;
    if (typeof record.base64 === "string") {
      return {
        mimeType: typeof record.mimeType === "string" ? record.mimeType : guessMime(undefined),
        base64: record.base64,
      };
    }
    if (data instanceof Uint8Array) {
      return { mimeType: "application/octet-stream", base64: bytesToBase64(data) };
    }
  }
  if (typeof data === "string") {
    return { mimeType: "text/plain; charset=utf-8", base64: utf8ToBase64(data) };
  }
  return {
    mimeType: "application/json",
    base64: utf8ToBase64(JSON.stringify(data)),
  };
}

function guessMime(name: string | undefined): string {
  if (name?.endsWith(".png")) return "image/png";
  if (name?.endsWith(".json")) return "application/json";
  if (name?.endsWith(".html")) return "text/html; charset=utf-8";
  if (name?.endsWith(".txt")) return "text/plain; charset=utf-8";
  return "application/octet-stream";
}

export function remapStack(stack: string | undefined): string | undefined {
  if (!stack) return undefined;
  const map = globalThis.__LINGXIA_TEST_SOURCE_MAP__;
  if (!map || typeof map !== "object") return stack;
  // The CLI injects the bundle map so we can rewrite lxdev-test:// frames.
  // A missing or unusable map falls back to the bundled locations.
  try {
    return remapWithMap(stack, map as SourceMapJson);
  } catch {
    return stack;
  }
}

interface SourceMapJson {
  sources?: string[];
  mappings?: string;
  sourcesContent?: Array<string | null>;
}

const VLQ_CHARS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

function decodeVlq(segment: string): number[] {
  const values: number[] = [];
  let i = 0;
  while (i < segment.length) {
    let result = 0;
    let shift = 0;
    let digit: number;
    do {
      const index = VLQ_CHARS.indexOf(segment[i] ?? "");
      if (index < 0) return values;
      i += 1;
      digit = index;
      result += (digit & 31) << shift;
      shift += 5;
    } while (digit & 32);
    const value = result >> 1;
    values.push(result & 1 ? -value : value);
  }
  return values;
}

function remapWithMap(stack: string, map: SourceMapJson): string {
  if (!map.mappings || !map.sources) return stack;
  const lines = map.mappings.split(";");
  const table: Array<{ col: number; source: number; srcLine: number; srcCol: number }[]> = [];
  let source = 0;
  let srcLine = 0;
  let srcCol = 0;
  for (const line of lines) {
    const entries: { col: number; source: number; srcLine: number; srcCol: number }[] = [];
    let col = 0;
    if (line.length > 0) {
      for (const segment of line.split(",")) {
        const decoded = decodeVlq(segment);
        if (decoded.length < 4) continue;
        col += decoded[0]!;
        source += decoded[1]!;
        srcLine += decoded[2]!;
        srcCol += decoded[3]!;
        entries.push({ col, source, srcLine, srcCol });
      }
    }
    table.push(entries);
  }

  return stack.replace(
    /(lxdev-test:\/\/[^:\s]+):(\d+):(\d+)/g,
    (match, _file: string, lineText: string, colText: string) => {
      const line = Number(lineText) - 1;
      const column = Number(colText) - 1;
      const entries = table[line];
      if (!entries || entries.length === 0) return match;
      let chosen = entries[0]!;
      for (const entry of entries) {
        if (entry.col <= column) chosen = entry;
        else break;
      }
      const sourcePath = map.sources?.[chosen.source] ?? "unknown";
      return `${sourcePath}:${chosen.srcLine + 1}:${chosen.srcCol + 1}`;
    },
  );
}
