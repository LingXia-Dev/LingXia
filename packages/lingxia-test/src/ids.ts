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

export function callerLocation(): { file: string; line: number; column: number } {
  const stack = new Error().stack ?? "";
  for (const raw of stack.split("\n").slice(2)) {
    if (
      /lingxia-test[/\\](dist|src)[/\\]/.test(raw) ||
      raw.includes("node:") ||
      raw.includes("node_modules")
    ) {
      continue;
    }
    const match =
      raw.match(/\((.+):(\d+):(\d+)\)/) ??
      raw.match(/at (file:\/\/.+):(\d+):(\d+)/) ??
      raw.match(/at (.+):(\d+):(\d+)/);
    if (!match) continue;
    return {
      file: stripFileUrl(match[1]!),
      line: Number(match[2]),
      column: Number(match[3]),
    };
  }
  return { file: "unknown", line: 0, column: 0 };
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
