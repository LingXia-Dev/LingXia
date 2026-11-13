// The compiler's source transforms only insert text. Each logs where, so a dev build can hand
// esbuild a map from the transformed text back to the author's, and runtime error columns name
// what the author wrote.

/**
 * Applies `inserts` (`[at, text]` in `source` offsets, already in splice order: descending `at`,
 * ties as the caller wants them) and, when `log` is given, records the stage for
 * {@link insertionMapComment}.
 */
export function spliceInserts(source, inserts, log) {
  let output = source;
  for (const [at, text] of inserts) output = output.slice(0, at) + text + output.slice(at);
  if (log) {
    const lengths = new Map();
    for (const [at, text] of inserts) lengths.set(at, (lengths.get(at) ?? 0) + text.length);
    log.push([...lengths].sort((a, b) => a[0] - b[0]));
  }
  return output;
}

/** The offset in a stage's input that `offset` in its output came from; undefined inside inserted text. */
function throughStage(offset, stage) {
  let shift = 0;
  for (const [at, length] of stage) {
    const start = at + shift;
    if (offset < start) break;
    if (offset < start + length) return undefined;
    shift += length;
  }
  return offset - shift;
}

const BASE64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
function vlq(value) {
  let rest = value < 0 ? (-value << 1) | 1 : value << 1, out = '';
  do {
    let digit = rest & 31;
    rest >>>= 5;
    if (rest) digit |= 32;
    out += BASE64[digit];
  } while (rest);
  return out;
}

function lineStarts(text) {
  const starts = [0];
  for (let i = 0; i < text.length; i++) if (text[i] === '\n') starts.push(i + 1);
  return starts;
}

function lineOf(starts, offset) {
  let low = 0, high = starts.length - 1;
  while (low < high) {
    const mid = (low + high + 1) >> 1;
    if (starts[mid] <= offset) low = mid; else high = mid - 1;
  }
  return low;
}

/**
 * A `sourceMappingURL` comment mapping each token of `output` to where it stands in `source`,
 * through the logged insertion `stages` (first applied first). Inserted tokens map nowhere.
 * Mappings are per token because a map lookup takes the nearest segment's position as is.
 */
export function insertionMapComment(source, output, stages, sourceName) {
  const sourceStarts = lineStarts(source);
  const lines = output.split('\n');
  let previous = { line: 0, column: 0 }, offset = 0;
  const encoded = lines.map(line => {
    const segments = [];
    let lastColumn = 0;
    for (const match of line.matchAll(/[\w$]+|[^\s\w$]/g)) {
      let at = offset + match.index;
      for (let stage = stages.length - 1; stage >= 0 && at !== undefined; stage--) at = throughStage(at, stages[stage]);
      if (at === undefined || at >= source.length) continue;
      const line = lineOf(sourceStarts, at), column = at - sourceStarts[line];
      segments.push(vlq(match.index - lastColumn) + vlq(0) + vlq(line - previous.line) + vlq(column - previous.column));
      lastColumn = match.index;
      previous = { line, column };
    }
    offset += line.length + 1;
    return segments.join(',');
  });
  const map = { version: 3, sources: [sourceName], sourcesContent: [source], names: [], mappings: encoded.join(';') };
  return `\n//# sourceMappingURL=data:application/json;base64,${Buffer.from(JSON.stringify(map)).toString('base64')}\n`;
}
