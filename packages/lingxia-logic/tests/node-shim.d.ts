// The two Node modules the TypeScript tests use; the package does not depend on @types/node.
declare module "node:test" {
  export function test(name: string, fn: () => void | Promise<void>): void;
}
declare module "node:assert/strict" {
  interface Assert {
    equal(actual: unknown, expected: unknown, message?: string): void;
    deepEqual(actual: unknown, expected: unknown, message?: string): void;
    rejects(promise: Promise<unknown>, expected?: RegExp | ((error: never) => boolean)): Promise<void>;
    throws(fn: () => unknown, expected?: RegExp | ((error: never) => boolean)): void;
  }
  const assert: Assert;
  export default assert;
}
