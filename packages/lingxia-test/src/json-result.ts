/** Sent as source to Logic and WebViews: keep this function self-contained. */
export function jsonResult(value: unknown, api: string, allowVoid = true, path = "result"): unknown {
  // A command can finish without a result. Nested undefined is never JSON.
  if (value === undefined && allowVoid) return undefined;
  const seen = new Set<object>();
  const visit = (item: unknown, path: string): unknown => {
    const fail = (reason: string): never => {
      throw new TypeError(`${api}: ${path}: ${reason}; use JSON values${allowVoid ? " or no top-level result" : ""}`);
    };
    if (item === null || typeof item === "string" || typeof item === "boolean") return item;
    if (typeof item === "number") return Number.isFinite(item) ? item : fail("non-finite number");
    if (typeof item !== "object") return fail(`${typeof item} is not JSON`);
    if (seen.has(item)) return fail("circular reference");
    const array = Array.isArray(item);
    const prototype = Object.getPrototypeOf(item);
    if (!array && prototype !== Object.prototype && prototype !== null) return fail("non-plain object is not JSON");
    seen.add(item);
    try {
      if (array) {
        const out: unknown[] = [];
        for (let index = 0; index < item.length; index++) {
          const descriptor = Object.getOwnPropertyDescriptor(item, String(index));
          if (!descriptor || !("value" in descriptor)) return fail(`array index ${index} is missing or an accessor`);
          out.push(visit(descriptor.value, `${path}[${index}]`));
        }
        // Extra array properties do not cross JSON either.
        if (Reflect.ownKeys(item).some((key) => key !== "length" &&
          !(typeof key === "string" && /^(0|[1-9][0-9]*)$/.test(key) && Number(key) < item.length))) {
          return fail("array has non-index properties");
        }
        return out;
      }
      const out: Record<string, unknown> = {};
      for (const key of Reflect.ownKeys(item)) {
        const descriptor = Object.getOwnPropertyDescriptor(item, key)!;
        if (!descriptor.enumerable) continue;
        if (typeof key !== "string") return fail("symbol key is not JSON");
        if (!("value" in descriptor)) return fail(`property ${key} is an accessor`);
        Object.defineProperty(out, key, {
          value: visit(descriptor.value, `${path}[${JSON.stringify(key)}]`),
          enumerable: true, writable: true, configurable: true,
        });
      }
      return out;
    } finally {
      seen.delete(item);
    }
  };
  return visit(value, path);
}
