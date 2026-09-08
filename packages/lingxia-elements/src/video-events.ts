/** Keep platform spelling differences out of the public DOM/payload contract. */
export function normalizeVideoEventDetail(event: string, payload: unknown): Record<string, unknown> {
  const detail = payload && typeof payload === "object" ? payload as Record<string, unknown> : {};
  if (["playrequest", "play", "playing", "pause", "stop", "ended", "waiting"].includes(event)) return {};
  if (event === "fullscreenchange") {
    return { ...detail, fullscreen: detail.fullscreen ?? detail.fullScreen ?? false };
  }
  if (event === "error") {
    return {
      ...detail,
      code: String(detail.code ?? "NATIVE_COMPONENT_COMMAND_FAILED"),
      message: String(detail.message ?? detail.errMsg ?? "Native video failed"),
    };
  }
  return detail;
}
