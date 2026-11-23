import { limits } from '../generated/contract.js';

const arity = { m: 2, l: 2, h: 1, v: 1, c: 6, s: 4, q: 4, t: 2, a: 7, z: 0 };

/** Segments `d` makes at least: each move, line, curve and close counts one, an arc one though it may become several. */
export function pathSegments(d) {
  let count = 0;
  for (const [, command, args] of d.matchAll(/([MmLlHhVvCcSsQqTtAaZz])([^MmLlHhVvCcSsQqTtAaZz]*)/g)) {
    const per = arity[command.toLowerCase()];
    const numbers = args.match(/[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?/g)?.length ?? 0;
    count += per ? Math.max(1, Math.floor(numbers / per)) : 1;
  }
  return count;
}

/** The problem with a path of `segments`, or undefined within the limit. */
export function pathSegmentsProblem(segments) {
  if (segments <= limits.vectorPathSegments) return undefined;
  return `UI_VECTOR: a path has ${segments} segments; the limit is ${limits.vectorPathSegments} per path: split it into several paths`;
}
