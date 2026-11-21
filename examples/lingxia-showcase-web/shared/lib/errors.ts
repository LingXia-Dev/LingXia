// `catch` binds `unknown`, so a rejection's message has to be read defensively.
// `lx` APIs reject with an Error, but a page also awaits plain promises and
// third-party code, and neither is guaranteed to.
export function errorMessage(error: unknown, fallback: string): string {
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === "string" && error) return error;
  if (typeof error === "object" && error !== null) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string" && message) return message;
  }
  return fallback;
}
