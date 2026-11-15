// Geometry kept between renders. A page re-renders whole when the theme changes, and a chart's
// paths depend on its data and encoding, never on the theme: colours are role names.

const kept = new WeakMap<object, Map<string, { key: string; value: unknown }>>();

/** `build()` once per `data` array and `key`; `slot` separates the things one chart keeps. */
export function remember<T>(data: object, slot: string, key: string, build: () => T): T {
  let slots = kept.get(data);
  if (!slots) kept.set(data, (slots = new Map()));
  const held = slots.get(slot);
  if (held && held.key === key) return held.value as T;
  const value = build();
  slots.set(slot, { key, value });
  return value;
}
