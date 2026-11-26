/** A failure as page data keeps it: `code` picks what a person reads (`errorText` in the View); `message` is English for the log. */
export type Failure = { readonly code: string; readonly message: string };

/** `catch` binds `unknown`; `lx` rejects with a coded error, other promises may not. */
export function failure(error: unknown): Failure {
  const e = error as { code?: unknown; message?: unknown } | null;
  return {
    code: typeof e?.code === 'string' ? e.code : 'E_INTERNAL',
    message: typeof e?.message === 'string' ? e.message : String(error),
  };
}

/** Logic's words for a failed action: the page's own text; the error's English message goes to the log. */
export function failureText(error: unknown, fallback: string): string {
  console.warn(fallback, error);
  return fallback;
}
