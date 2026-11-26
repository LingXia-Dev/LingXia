// Geometry kept between renders. A page re-renders whole when the theme changes, and a chart's
// paths depend on its data and encoding, never on the theme: colours are role names.

const kept = new WeakMap<object, Map<string, Map<string, unknown>>>();
// Charts that share one rows array each keep their own geometry; the oldest goes past this many.
const perSlot = 8;

/** `build()` once per `data` array, `slot` and `key`; `slot` separates the things one chart keeps. */
export function remember<T>(data: object, slot: string, key: string, build: () => T): T {
  let slots = kept.get(data);
  if (!slots) kept.set(data, (slots = new Map()));
  let held = slots.get(slot);
  if (!held) slots.set(slot, (held = new Map()));
  if (held.has(key)) {
    const value = held.get(key) as T;
    held.delete(key);
    held.set(key, value);
    return value;
  }
  const value = build();
  held.set(key, value);
  if (held.size > perSlot) held.delete(held.keys().next().value as string);
  return value;
}

const shown = new WeakMap<object, Set<string>>();

/**
 * Whether the chart `who` of `data` has a laid-out plot now or had one before. A chart draws in
 * once: one that comes back with the same rows, a list row scrolled back or a page returned to,
 * shows whole from its first frame.
 */
export function laidOutOnce(data: object, who: string, isLaidOut: boolean): boolean {
  let names = shown.get(data);
  if (isLaidOut) {
    if (!names) shown.set(data, (names = new Set()));
    names.add(who);
  }
  return isLaidOut || names?.has(who) === true;
}
