import { cx, type Child } from '@lingxia/view';
import { codeRecipe as r } from './Code.recipe';
import type { LineKind } from './diff';
import { highlight, type CodeRun, type TokenKind } from './highlight';

// A <text> holds 64 KiB, and the View builds about a thousand spans a frame: past either, code is several texts, stacked line by line.
const maxRuns = 800;
const maxBytes = 64 * 1024;

/** A string's UTF-8 length. */
function bytesOf(text: string): number {
  let bytes = 0;
  for (let index = 0; index < text.length; index += 1) {
    const unit = text.charCodeAt(index);
    if (unit < 0x80) bytes += 1;
    else if (unit < 0x800) bytes += 2;
    // A surrogate pair is one 4-byte character; each half counts 2.
    else if (unit >= 0xd800 && unit <= 0xdfff) bytes += 2;
    else bytes += 3;
  }
  return bytes;
}

/** A run of code and what colours it: a token's kind, a line's mark, or a diff header. No kind is plain text. */
export interface Run { text: string; token?: TokenKind; mark?: LineKind; isHeader?: boolean }

const alike = (a: Run, b: Run) => a.token === b.token && a.mark === b.mark && a.isHeader === b.isHeader;
const isPlain = (run: Run) => run.token === undefined && run.mark === undefined && !run.isHeader;

/** Lines of runs as texts' runs: neighbours with one class merged, a new text started before one would pass a limit. */
function chunks(lines: readonly (readonly Run[])[]): Run[][] {
  const out: Run[][] = [[]];
  let bytes = 0;
  const add = (into: Run[], run: Run) => {
    const last = into[into.length - 1];
    if (last !== undefined && alike(last, run)) last.text += run.text;
    else into.push({ ...run });
  };
  for (const line of lines) {
    const merged: Run[] = [];
    // A single line past the limit keeps its first runs' colours and goes on plain.
    line.forEach((run, index) => add(merged, index < maxRuns - 1 ? run : { text: run.text }));
    let current = out[out.length - 1];
    const joins = current.length > 0 && merged.length > 0 && alike(current[current.length - 1], merged[0]);
    const size = merged.reduce((sum, run) => sum + bytesOf(run.text), 0);
    if (current.length > 0 && (current.length + merged.length - (joins ? 1 : 0) > maxRuns || bytes + size > maxBytes)) {
      // The stack breaks the line, so a text ending on a break would show a blank line.
      const last = current[current.length - 1];
      last.text = last.text.slice(0, -1);
      if (last.text === '') current.pop();
      current = [];
      out.push(current);
      bytes = 0;
    }
    merged.forEach(run => add(current, run));
    bytes += size;
  }
  return out;
}

// Classes are looked up here, by key, so the build can follow every one to the recipe.
const shown = (runs: readonly Run[]) => runs.map((run, index) => (isPlain(run) ? run.text : (
  <span key={`${index}`} className={cx(run.token && r.syntax[run.token], run.mark && r.mark[run.mark], run.isHeader && r.headerLine.base)}>{run.text}</span>
)));

/** Code's runs cut at its line breaks, each line keeping its break. */
export function linesOf(runs: readonly CodeRun[], count: number): CodeRun[][] {
  const lines: CodeRun[][] = [[]];
  for (const run of runs) {
    const parts = run.text.split('\n');
    parts.forEach((part, index) => {
      if (index > 0) lines.push([]);
      const text = part + (index < parts.length - 1 ? '\n' : '');
      if (text !== '') lines[lines.length - 1].push({ text, kind: run.kind });
    });
  }
  while (lines.length < count) lines.push([]);
  return lines;
}

/** Lines of runs as one text, or past a text's limits several stacked. */
export function codeColumn(className: string, lineRuns: readonly (readonly Run[])[], hidden?: boolean): Child {
  const parts = chunks(lineRuns);
  if (parts.length === 1) return <text aria-hidden={hidden} className={className}>{shown(parts[0])}</text>;
  return <view aria-hidden={hidden} className={r.chunks.base}>{parts.map((runs, index) => <text key={`${index}`} aria-hidden={hidden} className={className}>{shown(runs)}</text>)}</view>;
}

/** Code coloured by its language's grammar, line by line. */
export function colouredLines(code: string, language: string | undefined): Run[][] {
  const count = code.split('\n').length;
  return linesOf(highlight(code, language), count).map(line => line.map(run => ({ text: run.text, token: run.kind })));
}
