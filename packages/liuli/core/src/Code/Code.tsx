import { cx, type Child } from '@lingxia/view';
import { codeRecipe as r } from './Code.recipe';
import { diffKinds, type LineKind } from './diff';
import { highlight, type CodeRun, type TokenKind } from './highlight';
import { dataOf } from '../internal/data';

export interface CodeProps {
  /** The code: a command, a file name, a value to type exactly. */
  text: string;
  'data-testid'?: string;
}

/** A few words of code inside a sentence. It goes in a `<text>`, among its words. */
export function Code({ text, 'data-testid': testId, ...attributes }: CodeProps) {
  return <span data-testid={testId} {...dataOf(attributes)} className={r.inline.base}>{text}</span>;
}

export interface CodeBlockProps {
  /** The code, with its own line breaks and indentation. */
  code: string;
  /** Names the block for assistive tech: "Shell command", "Example response". */
  label: string;
  /** Shown over the code: a file name, a language. */
  title?: string;
  /**
   * Colours the code by its language's grammar: `ts`, `tsx`, `js`, `json`, `css`, `html`, `xml`, `python`, `bash`, `yaml`, `toml`,
   * `rust`, `kotlin`, `swift`, `java`, `go`, `c`, `cpp` or `sql`, by name or file extension. Any other stays plain.
   */
  language?: string;
  /** At the end of the title's line: a Button or IconButton that copies the code. The app does the copying. */
  action?: Child;
  /** Numbers each line in a gutter before the code. @default false */
  hasLineNumbers?: boolean;
  /** The first line's number, so an excerpt shows its lines' numbers in the file. Below 1 reads as 1. @default 1 */
  startLineNumber?: number;
  /** Lines to mark, by the numbers the gutter shows (see `startLineNumber`). A number outside the code marks nothing; in a diff an added or removed line keeps its own tint. */
  highlightLines?: readonly number[];
  /**
   * Reads the code as a unified diff: a line starting `+` is added and `-` removed, `@@` starts a hunk, and a
   * `--- `/`+++ ` pair just before one names the files. The sign sits in its own column; the app's copy action
   * still has the whole text, signs included. @default false
   */
  isDiff?: boolean;
  /** Lets the code be selected. @default true */
  isSelectable?: boolean;
  className?: string;
  'data-testid'?: string;
}

const signs: Record<LineKind, string> = { context: ' ', added: '+', removed: '-', header: ' ' };

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
interface Run { text: string; token?: TokenKind; mark?: LineKind; isHeader?: boolean }

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
function linesOf(runs: readonly CodeRun[], count: number): CodeRun[][] {
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

/**
 * Lines of code as they were written: fixed-width, never wrapped, scrolling sideways. With numbers, a diff or
 * marked lines, each line's tint is a row behind the code, one line high, so the code stays one text to select.
 */
export function CodeBlock({
  code,
  label,
  title,
  language,
  action,
  hasLineNumbers = false,
  startLineNumber = 1,
  highlightLines,
  isDiff = false,
  isSelectable = true,
  className,
  'data-testid': testId, ...attributes
}: CodeBlockProps) {
  const lines = code.split('\n');
  const first = Math.max(1, Math.floor(startLineNumber));
  const marked = new Set(highlightLines ?? []);
  // A change keeps its own tint under a mark: in some packs the accent's tint is close to the removal's.
  const isChange = (kind: LineKind) => kind === 'added' || kind === 'removed';
  const kinds: LineKind[] = isDiff ? diffKinds(lines) : lines.map(() => 'context');
  const isLined = hasLineNumbers || isDiff || lines.some((_, index) => marked.has(first + index));
  const text = cx(r.text.base, isSelectable && r.text.selectable);
  // A diff's header lines are not code: they are left out of the grammar's reading and drawn quiet.
  const read = lines.map((line, index) => (kinds[index] === 'header' ? '' : isDiff ? line.slice(1) : line));
  const colored = linesOf(highlight(read.join('\n'), language), lines.length);
  const codeLines: Run[][] = lines.map((line, index) => {
    const end = index < lines.length - 1 ? '\n' : '';
    if (kinds[index] === 'header') return [{ text: line + end, isHeader: true }];
    return colored[index].map(run => ({ text: run.text, token: run.kind }));
  });
  const column = (className: string, lineRuns: readonly (readonly Run[])[], hidden?: boolean) => {
    const parts = chunks(lineRuns);
    if (parts.length === 1) return <text aria-hidden={hidden} className={className}>{shown(parts[0])}</text>;
    return <view aria-hidden={hidden} className={r.chunks.base}>{parts.map((runs, index) => <text key={`${index}`} aria-hidden={hidden} className={className}>{shown(runs)}</text>)}</view>;
  };
  const marks = (each: (kind: LineKind, index: number) => string) =>
    kinds.map((kind, index) => [{ text: `${each(kind, index)}${index < lines.length - 1 ? '\n' : ''}`, mark: kind }]);
  return (
    <view role="group" aria-label={label} data-testid={testId} {...dataOf(attributes)} className={cx(r.block.base, className)}>
      {(title !== undefined || action !== undefined) && (
        <view className={r.header.base}>
          <text className={r.title.base}>{title ?? ''}</text>
          {action}
        </view>
      )}
      {isLined ? (
        <scroll className={r.scroll.lined}>
          <view className={r.lines.base}>
            <view aria-hidden className={r.tints.base}>
              {kinds.map((kind, index) => (
                <view key={`${index}`} className={cx(r.tint.base, marked.has(first + index) && !isChange(kind) ? r.tint.highlighted : r.tint[kind])} />
              ))}
            </view>
            {hasLineNumbers && column(r.gutter.base, marks((_, index) => `${first + index}`), true)}
            {isDiff && column(cx(r.signs.base, hasLineNumbers && r.signs.numbered), marks(kind => signs[kind]))}
            {column(cx(text, r.text.lined, isDiff ? r.text.afterSign : r.text.start), codeLines)}
          </view>
        </scroll>
      ) : (
        <scroll className={r.scroll.base}>
          {column(text, codeLines)}
        </scroll>
      )}
    </view>
  );
}
