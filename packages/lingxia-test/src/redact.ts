/**
 * `--arg` values end up in every report, and reports get shared. A key that
 * looks like a credential, or one passed with `lxdev test --secret-arg`, is
 * written as `***`; the spec still reads the real value from `t.args`.
 */

export const REDACTED = "***";

/** Reserved arg carrying the JSON list of `--secret-arg` keys. */
export const SECRET_ARGS_KEY = "secretArgs";

const SECRET_KEY = /pass(word)?|secret|token|api[_-]?key|credential/i;

/** Shorter values are too likely to collide with ordinary report text. */
const MIN_SCRUB_LENGTH = 4;

export interface Redactor {
  /** Args with secret values replaced, for `meta.args`. */
  args(args: Record<string, string>): Record<string, string>;
  /** Deep copy with every secret value masked inside strings. */
  deep<T>(value: T): T;
}

export function isSecretKey(key: string, explicit: ReadonlySet<string> = new Set()): boolean {
  return explicit.has(key) || SECRET_KEY.test(key);
}

export function createRedactor(args: Record<string, string>): Redactor {
  const explicit = new Set<string>();
  const listed = args[SECRET_ARGS_KEY];
  if (listed) {
    try {
      const parsed: unknown = JSON.parse(listed);
      if (Array.isArray(parsed)) for (const key of parsed) if (typeof key === "string") explicit.add(key);
    } catch {
      for (const key of listed.split(",")) if (key.trim()) explicit.add(key.trim());
    }
  }
  const values = Object.entries(args)
    .filter(([key, value]) => key !== SECRET_ARGS_KEY && isSecretKey(key, explicit) && value.length >= MIN_SCRUB_LENGTH)
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
        out[key] = key !== SECRET_ARGS_KEY && isSecretKey(key, explicit) ? REDACTED : value;
      }
      return out;
    },
    deep<T>(value: T): T {
      return values.length === 0 ? value : (walk(value, new WeakMap()) as T);
    },
  };
}
