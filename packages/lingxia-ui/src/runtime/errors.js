// Errors keep their code prefix (`UI_PROP: …`); render errors gain the JSX
// sites and components they passed through.
export function located(error, frame) {
  const base = error instanceof Error ? error : Error(String(error));
  if (!base.uiFrames) { base.uiFrames = []; base.uiMessage = base.message; }
  if (base.uiFrames.length < 12) base.uiFrames.push(frame);
  base.message = [base.uiMessage, ...base.uiFrames.map(line => `  at ${line}`)].join('\n');
  return base;
}

export function bridgeError(code, message, data) {
  const error = new Error(message ?? code);
  error.name = 'LxBridgeError'; error.code = code;
  if (data !== undefined) error.data = data;
  return error;
}

/** The `{ code, message }` a resource exposes for a failure. */
export function resourceError(error) {
  if (typeof error?.code === 'string') return Object.freeze({ code: error.code, message: String(error.message ?? error.code) });
  const text = String(error?.uiMessage ?? error?.message ?? error);
  const match = text.match(/^([A-Z][A-Z0-9_]+): ([\s\S]*)$/);
  return Object.freeze(match ? { code: match[1], message: match[2] } : { code: 'UI_ERROR', message: text });
}
