const hasOwn = Object.prototype.hasOwnProperty;
const objectTag = Object.prototype.toString;

export function safeString(value: unknown): string {
  try {
    return String(value);
  } catch {
    return "[Unstringifiable]";
  }
}

export function truncate(value: unknown, limit: number): string {
  const text = safeString(value);
  return text.length <= limit ? text : `${text.slice(0, limit)}…`;
}

function isTypedArray(value: unknown): value is ArrayLike<number> {
  return (
    typeof ArrayBuffer !== "undefined" &&
    ArrayBuffer.isView(value) &&
    objectTag.call(value) !== "[object DataView]"
  );
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  if (objectTag.call(value) !== "[object Object]") return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

export function formatValue(value: unknown): string {
  const seen: unknown[] = [];
  function format(current: unknown, depth: number): string {
    if (current === null) return "null";
    const type = typeof current;
    if (type === "undefined") return "undefined";
    if (type === "string") return JSON.stringify(truncate(current, 160));
    if (type === "number" || type === "boolean" || type === "bigint") {
      return String(current) + (type === "bigint" ? "n" : "");
    }
    if (type === "symbol") return truncate(String(current), 160);
    if (type === "function") {
      const name = (current as { name?: string }).name || "anonymous";
      return `[Function ${name}]`;
    }
    if (depth >= 4) return "[…]";
    if (seen.includes(current)) return "[Circular]";
    seen.push(current);
    try {
      if (Array.isArray(current)) {
        const limit = Math.min(current.length, 12);
        const parts = [];
        for (let i = 0; i < limit; i += 1) {
          parts.push(hasOwn.call(current, i) ? format(current[i], depth + 1) : "<empty>");
        }
        if (current.length > limit) parts.push("…");
        return `[${parts.join(", ")}]`;
      }
      if (isTypedArray(current)) {
        const limit = Math.min(current.length, 12);
        const parts = [];
        for (let i = 0; i < limit; i += 1) parts.push(String(current[i]));
        if (current.length > limit) parts.push("…");
        return `${objectTag.call(current).slice(8, -1)}[${parts.join(", ")}]`;
      }
      if (current instanceof Error) {
        return `${current.name}(${JSON.stringify(truncate(current.message, 160))})`;
      }
      if (objectTag.call(current) === "[object RegExp]") return String(current);
      if (isPlainObject(current)) {
        const keys = Object.keys(current);
        const limit = Math.min(keys.length, 12);
        const parts = [];
        for (let i = 0; i < limit; i += 1) {
          const key = keys[i]!;
          let formatted: string;
          try {
            formatted = format(current[key], depth + 1);
          } catch {
            formatted = "[Unformattable]";
          }
          parts.push(`${JSON.stringify(truncate(key, 80))}: ${formatted}`);
        }
        if (keys.length > limit) parts.push("…");
        return `{${parts.join(", ")}}`;
      }
      return objectTag.call(current);
    } finally {
      seen.pop();
    }
  }
  try {
    return truncate(format(value, 0), 2000);
  } catch {
    return "[Unformattable]";
  }
}

const BASE64_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/**
 * Encode bytes without `btoa`. The Logic runtime's `btoa` UTF-8-encodes its
 * argument instead of reading it as a latin-1 byte string, which
 * double-encodes every non-ASCII character in the report. Owning the encoder
 * keeps the artifacts byte-exact on every engine.
 */
export function bytesToBase64(bytes: Uint8Array): string {
  let out = "";
  const remainder = bytes.length % 3;
  const end = bytes.length - remainder;
  for (let i = 0; i < end; i += 3) {
    const chunk = (bytes[i]! << 16) | (bytes[i + 1]! << 8) | bytes[i + 2]!;
    out += BASE64_ALPHABET[(chunk >> 18) & 63];
    out += BASE64_ALPHABET[(chunk >> 12) & 63];
    out += BASE64_ALPHABET[(chunk >> 6) & 63];
    out += BASE64_ALPHABET[chunk & 63];
  }
  if (remainder === 1) {
    const chunk = bytes[end]! << 16;
    out += BASE64_ALPHABET[(chunk >> 18) & 63];
    out += BASE64_ALPHABET[(chunk >> 12) & 63];
    out += "==";
  } else if (remainder === 2) {
    const chunk = (bytes[end]! << 16) | (bytes[end + 1]! << 8);
    out += BASE64_ALPHABET[(chunk >> 18) & 63];
    out += BASE64_ALPHABET[(chunk >> 12) & 63];
    out += BASE64_ALPHABET[(chunk >> 6) & 63];
    out += "=";
  }
  return out;
}

export function utf8ToBase64(text: string): string {
  return bytesToBase64(new TextEncoder().encode(text));
}

export function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

export function cssEscape(value: string): string {
  if (typeof CSS !== "undefined" && typeof CSS.escape === "function") {
    return CSS.escape(value);
  }
  let out = "";
  for (let i = 0; i < value.length; i += 1) {
    const code = value.charCodeAt(i);
    const ch = value[i]!;
    if (i === 0 && code >= 48 && code <= 57) {
      out += `\\${code.toString(16)} `;
      continue;
    }
    if (
      (code >= 48 && code <= 57) ||
      (code >= 65 && code <= 90) ||
      (code >= 97 && code <= 122) ||
      ch === "-" ||
      ch === "_"
    ) {
      out += ch;
      continue;
    }
    if (code >= 0x80) {
      out += ch;
      continue;
    }
    out += `\\${ch}`;
  }
  return out;
}
