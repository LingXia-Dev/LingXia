// The contract — exported so the mock and the lxapp's typed `lx.cloud.invoke`
// reuse it. Defined inline (lingxiao resolves types per-file; it does not follow
// imports), so this file is `lingxiao build`-ready on its own.
export type HelloInput = {
  /** Who to greet. */
  name?: string;
};

export type HelloOutput = {
  /** The greeting to display. */
  greeting: string;
};

/**
 * Greet a user. Live implementation — replace the body with real logic
 * (e.g. call `lx.clients.*`). Same contract as the mock in ../../mocks/hello.ts.
 * @param input.name who to greet
 * @returns the greeting
 */
async function hello(
  request: LxFunctionRequest<HelloInput>,
): Promise<HelloOutput> {
  return { greeting: `Hello, ${request.input.name ?? "world"}! (live)` };
}

export const helloFn = lx.fn("hello", hello);
