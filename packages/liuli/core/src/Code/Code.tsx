import { cx, type Child } from '@lingxia/view';
import { codeRecipe as r } from './Code.recipe';
import { diffKinds, type LineKind } from './diff';
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

/**
 * Lines of code as they were written: fixed-width, never wrapped, scrolling sideways. With numbers, a diff or
 * marked lines, each line's tint is a row behind the code, one line high, so the code stays one text to select.
 */
export function CodeBlock({
  code,
  label,
  title,
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
            {hasLineNumbers && (
              <text aria-hidden className={r.gutter.base}>
                {kinds.map((kind, index) => (
                  <span key={`${index}`} className={r.mark[kind]}>{`${first + index}${index < lines.length - 1 ? '\n' : ''}`}</span>
                ))}
              </text>
            )}
            {isDiff && (
              <text className={cx(r.signs.base, hasLineNumbers && r.signs.numbered)}>
                {kinds.map((kind, index) => (
                  <span key={`${index}`} className={r.mark[kind]}>{`${signs[kind]}${index < lines.length - 1 ? '\n' : ''}`}</span>
                ))}
              </text>
            )}
            <text className={cx(text, r.text.lined, isDiff ? r.text.afterSign : r.text.start)}>
              {lines.map((line, index) => {
                const shown = isDiff && kinds[index] !== 'header' ? line.slice(1) : line;
                const end = index < lines.length - 1 ? '\n' : '';
                return kinds[index] === 'header'
                  ? <span key={`${index}`} className={r.headerLine.base}>{shown + end}</span>
                  : <span key={`${index}`}>{shown + end}</span>;
              })}
            </text>
          </view>
        </scroll>
      ) : (
        <scroll className={r.scroll.base}>
          <text className={text}>{code}</text>
        </scroll>
      )}
    </view>
  );
}
