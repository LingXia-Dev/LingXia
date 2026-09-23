/**
 * `--arg` values end up in every report, and reports get shared.
 *
 * Two different guarantees, on purpose:
 * - An arg passed with `lxdev test --secret-arg` is a declared secret. Its
 *   value is masked wherever it appears: `meta.args`, events, case records and
 *   text attachments.
 * - An ordinary `--arg` whose key is named like a credential (`password`,
 *   `apiKey`, `DB_TOKEN`) only has its entry in `meta.args` shown as `***`. Its
 *   value is not searched for elsewhere: a guess from the name must not blank
 *   every `1000` in a report because someone passed `maxTokens=1000`.
 *
 * The spec always reads the real value from `t.args`.
 */

import { bytesToBase64 } from "./format.js";

export const REDACTED = "***";

/** Shorter values are too likely to collide with ordinary report text. */
const MIN_SCRUB_LENGTH = 4;

/** A key whose last word is one of these names a credential. */
const SECRET_WORDS = new Set([
  "password",
  "passwd",
  "pwd",
  "passphrase",
  "secret",
  "token",
  "credential",
  "credentials",
  "apikey",
]);

/** Two-word endings that name a credential (`apiKey`, `PRIVATE_KEY`). */
const SECRET_PAIRS = new Set(["api key", "private key"]);

/**
 * Split a key into lower-case words at `_`, `-`, `.`, spaces and camelCase
 * boundaries: `DB_PASSWORD` → db password, `apiKey` → api key,
 * `APIKey` → api key, `passWithNoTests` → pass with no tests.
 */
export function keyWords(key: string): string[] {
  return key
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/([A-Z]+)([A-Z][a-z])/g, "$1 $2")
    .split(/[^A-Za-z0-9]+/)
    .filter((word) => word.length > 0)
    .map((word) => word.toLowerCase());
}

/**
 * Whether an arg key names a credential. Only the key's last word counts, as a
 * whole word: `authToken` and `DB_PASSWORD` do; `tokenCount`, `maxTokens`,
 * `bypassCache`, `passport` and `passWithNoTests` do not.
 */
export function looksSecretKey(key: string): boolean {
  const words = keyWords(key);
  const last = words[words.length - 1];
  if (last === undefined) return false;
  if (SECRET_WORDS.has(last)) return true;
  const pair = words.length >= 2 ? `${words[words.length - 2]} ${last}` : "";
  return SECRET_PAIRS.has(pair);
}

export interface Redactor {
  /** Args for `meta.args`: declared secrets and credential-named keys as `***`. */
  args(args: Record<string, string>): Record<string, string>;
  /** Deep copy with every declared secret value masked inside strings. */
  deep<T>(value: T): T;
  /** An attachment payload with declared secret values masked in its text. */
  attachment(data: unknown): unknown;
}

/**
 * @param args  the user args (`t.args`)
 * @param secretKeys  keys passed with `--secret-arg`
 */
export function createRedactor(
  args: Record<string, string>,
  secretKeys: Iterable<string> = [],
): Redactor {
  const declared = new Set(secretKeys);
  const values = Object.entries(args)
    .filter(([key, value]) => declared.has(key) && value.length >= MIN_SCRUB_LENGTH)
    .map(([, value]) => value)
    // Longest first, so a secret containing another is masked whole.
    .sort((a, b) => b.length - a.length);

  const scrub = (text: string): string => {
    let out = text;
    for (const value of values) if (out.includes(value)) out = out.split(value).join(REDACTED);
    return out;
  };
  const walk = (value: unknown, seen: WeakMap<object, unknown>): unknown => {
    if (typeof value === "string") return scrub(value);
    if (!value || typeof value !== "object") return value;
    if (value instanceof Uint8Array) return value;
    if (seen.has(value)) return seen.get(value);
    if (Array.isArray(value)) {
      const out: unknown[] = [];
      seen.set(value, out);
      for (const item of value) out.push(walk(item, seen));
      return out;
    }
    const out: Record<string, unknown> = {};
    seen.set(value, out);
    for (const [key, item] of Object.entries(value as Record<string, unknown>)) out[key] = walk(item, seen);
    return out;
  };

  return {
    args(input) {
      const out: Record<string, string> = {};
      for (const [key, value] of Object.entries(input)) {
        out[key] = declared.has(key) || looksSecretKey(key) ? REDACTED : value;
      }
      return out;
    },
    deep<T>(value: T): T {
      return values.length === 0 ? value : (walk(value, new WeakMap()) as T);
    },
    attachment(data) {
      if (values.length === 0) return data;
      if (data && typeof data === "object" && !(data instanceof Uint8Array)) {
        const record = data as Record<string, unknown>;
        // A pre-encoded payload: only text is searched; images stay byte-exact.
        if (typeof record.base64 === "string") {
          const mimeType = typeof record.mimeType === "string" ? record.mimeType : "";
          if (!isText(mimeType)) return data;
          const text = decodeUtf8(record.base64);
          if (text === undefined) return data;
          const scrubbed = scrub(text);
          if (scrubbed === text) return data;
          return { ...record, base64: bytesToBase64(new TextEncoder().encode(scrubbed)) };
        }
      }
      return walk(data, new WeakMap());
    },
  };
}

function isText(mimeType: string): boolean {
  return (
    mimeType.startsWith("text/") ||
    mimeType.startsWith("application/json") ||
    mimeType.startsWith("application/xml") ||
    /\+(json|xml)\b/.test(mimeType)
  );
}

const BASE64_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

function decodeUtf8(base64: string): string | undefined {
  if (typeof TextDecoder === "undefined") return undefined;
  const clean = base64.replace(/[\s=]/g, "");
  const bytes = new Uint8Array(Math.floor((clean.length * 3) / 4));
  let buffer = 0;
  let bits = 0;
  let index = 0;
  for (const char of clean) {
    const value = BASE64_ALPHABET.indexOf(char);
    if (value < 0) return undefined;
    buffer = (buffer << 6) | value;
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      bytes[index++] = (buffer >> bits) & 0xff;
    }
  }
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes.subarray(0, index));
  } catch {
    return undefined;
  }
}
