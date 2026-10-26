// Errors keep their code prefix (`UI_PROP: …`); render errors gain the JSX
// sites and components they passed through.
export function located(error, frame) {
  const base = error instanceof Error ? error : Error(String(error));
  if (!base.uiFrames) {
    base.uiFrames = []; base.uiMessage = base.message;
    // The author line that threw, when the build kept a source map (dev builds).
    const site = authorSite(base.stack);
    if (site) base.uiFrames.push(site);
  }
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

// Dev builds carry the page bundle's source map: `{ sources, mappings }`, with
// runtime and package sources left out (`null`), decoded on the first error.
let sourceMap = null, decoded = null;
export function configureSourceMap(map) { sourceMap = map; decoded = null; }

const base64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
function decode() {
  const lines = [];
  let source = 0, line = 0, column = 0;
  for (const text of sourceMap.mappings.split(';')) {
    const segments = [];
    let generated = 0;
    for (const segment of text.split(',')) {
      if (!segment) continue;
      const values = [];
      let value = 0, shift = 0;
      for (const char of segment) {
        const digit = base64.indexOf(char);
        value += (digit & 31) << shift;
        if (digit & 32) shift += 5;
        else { values.push(value & 1 ? -(value >>> 1) : value >>> 1); value = 0; shift = 0; }
      }
      generated += values[0];
      if (values.length < 4) continue;
      source += values[1]; line += values[2]; column += values[3];
      segments.push([generated, source, line, column]);
    }
    lines.push(segments);
  }
  return lines;
}

/** `file:line:col` of the first stack frame inside the author's sources. */
export function authorSite(stack) {
  if (!sourceMap || typeof stack !== 'string') return undefined;
  decoded ??= decode();
  for (const match of stack.matchAll(/:(\d+):(\d+)\)?\s*$/gm)) {
    const segments = decoded[Number(match[1]) - 1];
    if (!segments?.length) continue;
    const column = Number(match[2]) - 1;
    let best = segments[0];
    for (const segment of segments) if (segment[0] <= column) best = segment;
    const file = sourceMap.sources[best[1]];
    if (file) return `${file}:${best[2] + 1}:${best[3] + 1 + Math.max(0, column - best[0])}`;
  }
  return undefined;
}

/** An error thrown outside render (a handler) gains its author line too. */
export function sited(error) {
  if (!(error instanceof Error) || error.uiFrames) return error;
  const site = authorSite(error.stack);
  if (!site) return error;
  error.uiFrames = [site]; error.uiMessage = error.message;
  error.message = `${error.message}\n  at ${site}`;
  return error;
}
