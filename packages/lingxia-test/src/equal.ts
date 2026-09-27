/**
 * Pairs assumed equal while their contents are compared, so cycles
 * terminate, and the order they were assumed in.
 */
interface Assumed {
  pairs: Map<object, Set<object>>;
  trail: [object, object][];
}

/** Deep equality for `toEqual` and schema `enum`/`const`/`uniqueItems`:
 * Dates by time, RegExps by source, Maps and Sets by contents (each element
 * or entry matched once), cycles safe. */
export function isEqual(a: unknown, b: unknown): boolean {
  return equal(a, b, { pairs: new Map(), trail: [] });
}

function equal(a: unknown, b: unknown, assumed: Assumed): boolean {
  if (Object.is(a, b)) return true;
  if (typeof a !== typeof b) return false;
  if (a === null || b === null) return a === b;
  if (typeof a !== "object" || typeof b !== "object") return false;
  if (a instanceof Date || b instanceof Date) {
    return a instanceof Date && b instanceof Date && a.getTime() === b.getTime();
  }
  if (a instanceof RegExp || b instanceof RegExp) {
    return a instanceof RegExp && b instanceof RegExp && String(a) === String(b);
  }
  if ((a instanceof Map) !== (b instanceof Map)) return false;
  if ((a instanceof Set) !== (b instanceof Set)) return false;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  // Coinductive: a pair already under comparison counts as equal. When the
  // comparison fails, it and every pair concluded under it are withdrawn,
  // so a Set match that backtracks leaves no false pair behind.
  let pairs = assumed.pairs.get(a);
  if (pairs?.has(b)) return true;
  if (!pairs) assumed.pairs.set(a, pairs = new Set());
  pairs.add(b);
  const mark = assumed.trail.length;
  assumed.trail.push([a, b]);
  if (contentsEqual(a, b, assumed)) return true;
  for (const [x, y] of assumed.trail.splice(mark)) assumed.pairs.get(x)?.delete(y);
  return false;
}

function contentsEqual(a: object, b: object, assumed: Assumed): boolean {
  if (a instanceof Map && b instanceof Map) {
    if (a.size !== b.size) return false;
    const left: [unknown, unknown][] = [];
    const right: [unknown, unknown][] = [];
    for (const [key, value] of a) {
      if (isObject(key)) left.push([key, value]);
      else if (!b.has(key) || !equal(value, b.get(key), assumed)) return false;
    }
    for (const entry of b) if (isObject(entry[0])) right.push(entry);
    return matchAll(left, right, ([ka, va], [kb, vb]) => equal(ka, kb, assumed) && equal(va, vb, assumed));
  }
  if (a instanceof Set && b instanceof Set) {
    if (a.size !== b.size) return false;
    const left: object[] = [];
    const right: object[] = [];
    for (const value of a) {
      if (isObject(value)) left.push(value);
      else if (!b.has(value)) return false;
    }
    for (const value of b) if (isObject(value)) right.push(value);
    return matchAll(left, right, (x, y) => equal(x, y, assumed));
  }
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) return false;
    return a.every((item, index) => equal(item, b[index], assumed));
  }
  const aKeys = Object.keys(a);
  const bKeys = Object.keys(b);
  if (aKeys.length !== bKeys.length) return false;
  return aKeys.every((key) =>
    Object.prototype.hasOwnProperty.call(b, key) &&
    equal((a as Record<string, unknown>)[key], (b as Record<string, unknown>)[key], assumed)
  );
}

function isObject(value: unknown): value is object {
  return typeof value === "object" && value !== null;
}

/**
 * A one-to-one pairing of `left` with `right` under `same` (bipartite
 * matching by augmenting paths): each right element answers for at most
 * one left element, and an early greedy choice is revisited when a later
 * element needs it.
 */
function matchAll<T>(left: T[], right: T[], same: (x: T, y: T) => boolean): boolean {
  if (left.length !== right.length) return false;
  const candidates = left.map((x) => right.flatMap((y, index) => (same(x, y) ? [index] : [])));
  const owner = new Array<number>(right.length).fill(-1);
  const assign = (i: number, visited: boolean[]): boolean => {
    for (const j of candidates[i]!) {
      if (visited[j]) continue;
      visited[j] = true;
      if (owner[j] === -1 || assign(owner[j]!, visited)) {
        owner[j] = i;
        return true;
      }
    }
    return false;
  };
  return left.every((_, i) => candidates[i]!.length > 0 && assign(i, new Array<boolean>(right.length).fill(false)));
}
