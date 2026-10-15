// `catch` binds `unknown`, so a rejection's message has to be read defensively.
// `lx` APIs reject with an Error, but a page also awaits plain promises and
// third-party code, and neither is guaranteed to.
export function errorMessage(error: unknown, fallback: string): string {
  if (error instanceof Error && error.message) return withDetail(error.message, error);
  if (typeof error === "string" && error) return error;
  if (typeof error === "object" && error !== null) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string" && message) return withDetail(message, error);
  }
  return fallback;
}

// An `lx` rejection's message is the localized category ("Invalid parameter");
// the reason a user can act on is in `data.detail`.
function withDetail(message: string, error: object): string {
  const detail = (error as { data?: { detail?: unknown } }).data?.detail;
  if (typeof detail !== "string" || !detail || message.includes(detail)) return message;
  return `${message}: ${detail}`;
}
