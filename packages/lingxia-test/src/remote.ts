/**
 * Function-form eval: a spec passes a function, the fixture sends its source.
 *
 * Both targets already accept a script string, but they classify it with
 * different heuristics (Logic tries an expression, then a function body; the
 * WebView awaits an expression unless it starts with a statement keyword).
 * The script built here is one parenthesised call expression, which both read
 * the same way, so a spec never meets either heuristic.
 */

export type RemoteTarget = "logic" | "page";

const FUNCTION_SOURCE =
  /^\s*(?:async\s*)?(?:function\b|\(|[A-Za-z_$][\w$]*\s*=>)/;

/** `fn.toString()`, rejecting what cannot be re-evaluated as an expression. */
export function functionSource(fn: unknown, api: string): string {
  if (typeof fn !== "function") {
    throw new TypeError(`${api} expects a function or { script }`);
  }
  const source = Function.prototype.toString.call(fn);
  if (/\{\s*\[native code\]\s*\}\s*$/.test(source)) {
    throw new TypeError(
      `${api}: a bound or native function has no source to send; pass an inline function`,
    );
  }
  if (!FUNCTION_SOURCE.test(source)) {
    throw new TypeError(
      `${api}: pass an arrow or function expression, not a method or class ` +
        `(received ${JSON.stringify(source.slice(0, 40))})`,
    );
  }
  return source;
}

function argsLiteral(args: readonly unknown[], api: string): string {
  let json: string | undefined;
  try {
    json = JSON.stringify(args);
  } catch (error) {
    throw new TypeError(`${api}: arguments must be JSON values (${(error as Error).message})`);
  }
  return json ?? "[]";
}

/**
 * Logic: a direct `eval` in the runtime, so `lx` here is the runtime's
 * recording binding and `lx.*` coverage keeps working. `getApp` and
 * `getCurrentPages` are read defensively: a missing one must surface as
 * `undefined` in the scope, not as a ReferenceError blamed on the spec.
 */
export function logicScript(fn: unknown, args: readonly unknown[], api = "t.app.logic.eval"): string {
  const source = functionSource(fn, api);
  return [
    "((__lxFn, __lxArgs) => __lxFn({",
    "  lx,",
    '  getApp: typeof getApp === "function" ? getApp : undefined,',
    '  getCurrentPages: typeof getCurrentPages === "function" ? getCurrentPages : undefined,',
    "}, ...__lxArgs))(",
    source,
    `, ${argsLiteral(args, api)})`,
  ].join("\n");
}

/** WebView: evaluated as `await (<expression>)`. */
export function pageScript(fn: unknown, args: readonly unknown[], api = "t.app.view.eval"): string {
  const source = functionSource(fn, api);
  return [
    "((__lxFn, __lxArgs) => __lxFn({ document, window }, ...__lxArgs))(",
    source,
    `, ${argsLiteral(args, api)})`,
  ].join("\n");
}

/**
 * The usual way a function-form eval fails: it closed over something from
 * the spec, which does not exist where it runs. Say so, keeping the original
 * error's code and data.
 */
export function explainRemoteError(error: unknown, api: string, target: RemoteTarget): unknown {
  const message = error instanceof Error ? error.message : String(error);
  const name = error instanceof Error ? error.name : "";
  if (name !== "ReferenceError" && !/\bReferenceError\b/.test(message)) return error;
  const where = target === "logic" ? "the app's Logic runtime" : "the page WebView";
  const explained = new Error(
    `${message}\n${api}(fn) runs fn in ${where} from its source text, so it cannot ` +
      "use variables, imports or helpers of the spec. Pass values as JSON " +
      `arguments: ${api}((scope, value) => ..., value).`,
  ) as Error & { code?: unknown; data?: unknown; cause?: unknown };
  // Named for what it is, so `t.waitFor` fails fast on it instead of retrying.
  explained.name = "ReferenceError";
  const details = error as { code?: unknown; data?: unknown };
  if (details && typeof details === "object") {
    if (details.code !== undefined) explained.code = details.code;
    if (details.data !== undefined) explained.data = details.data;
  }
  explained.cause = error;
  return explained;
}

/** Report detail for a function-form eval: its source, whitespace-collapsed. */
export function functionDetail(fn: unknown): string {
  try {
    return Function.prototype.toString.call(fn).replace(/\s+/g, " ").trim();
  } catch {
    return "";
  }
}
