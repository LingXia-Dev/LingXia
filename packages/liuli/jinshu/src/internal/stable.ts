type AnyFn = (...args: never[]) => unknown;
const held = new WeakMap<object, { latest: Record<string, AnyFn | undefined>; fns: Record<string, AnyFn> }>();

/**
 * The given callbacks with identities that last across renders, each calling the latest render's
 * version: a block whose other props are unchanged then keeps its last render. `anchor` is any
 * object that lives as long as the component, such as a `useLocal` binding.
 */
export function stableCallbacks<T extends Record<string, AnyFn | undefined>>(anchor: object, latest: T): T {
  let entry = held.get(anchor);
  if (!entry) held.set(anchor, (entry = { latest, fns: {} }));
  entry.latest = latest;
  const out: Record<string, AnyFn> = {};
  for (const key of Object.keys(latest)) {
    if (latest[key] === undefined) continue;
    const current = entry;
    out[key] = entry.fns[key] ??= ((...args: never[]) => current.latest[key]?.(...args)) as AnyFn;
  }
  return out as T;
}
