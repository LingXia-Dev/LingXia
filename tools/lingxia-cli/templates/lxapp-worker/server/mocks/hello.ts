import type { HelloInput, HelloOutput } from "../src/functions/hello";

// Mock provider for `hello` — what `lingxia dev` serves by default. Returns a
// timestamped greeting so each call (and each home-button click) visibly
// changes. Same contract as the live worker function in src/functions/hello.ts.
export const hello = lx.fn(
  "hello",
  (request: { input?: HelloInput }): HelloOutput => ({
    greeting: `Hello, ${request.input?.name ?? "world"}! 👋 (mock @ ${Date.now()})`,
  }),
);
