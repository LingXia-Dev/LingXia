import { cx, useId, useLocal, type Child } from '@lingxia/view';
import { IconButton } from '../IconButton/IconButton';
import { liuliText } from '../i18n/text';
import { reportFailure, settleFlag } from '../internal/settle';
import { codeRecipe as r } from './Code.recipe';
import { diffKinds, type LineKind } from './diff';
import { highlight } from './highlight';
import { codeColumn, linesOf, type Run } from './lines';
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

/** Wrapped code has no line columns: numbers, signs and marks are one line high, and a wrapped line is several. */
export type CodeBlockLayout =
  | {
      /** Numbers each line in a gutter before the code. @default false */
      hasLineNumbers?: boolean;
      /** The first line's number, so an excerpt shows its lines' numbers in the file. Below 1 reads as 1. @default 1 */
      startLineNumber?: number;
      /** Lines to mark, by the numbers the gutter shows (see `startLineNumber`). A number outside the code marks nothing; in a diff an added or removed line keeps its own tint. */
      highlightLines?: readonly number[];
      /**
       * Reads the code as a unified diff: a line starting `+` is added and `-` removed, `@@` starts a hunk, and a
       * `--- `/`+++ ` pair just before one names the files. The sign sits in its own column; `onCopy` and the
       * app's action still get the whole text, signs included. @default false
       */
      isDiff?: boolean;
      /** Long lines wrap at the block's width instead of scrolling sideways. @default false */
      isWrapped?: false;
    }
  | {
      isWrapped: true;
      hasLineNumbers?: never;
      startLineNumber?: never;
      highlightLines?: never;
      isDiff?: never;
    };

export type CodeBlockProps = CodeBlockLayout & {
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
  /**
   * Draws a copy button at the end of the title's line and runs with the code when it is pressed. Liuli has no
   * clipboard: pass the call that puts text on it. When that call returns (a promise, once it resolves) the
   * button says "Copied" for a moment.
   */
  onCopy?: (code: string) => void | Promise<unknown>;
  /** At the end of the title's line, before the copy button: a Button or IconButton the app draws. */
  action?: Child;
  /** The code area's tallest height, in points; longer code scrolls inside it, under the title's line. */
  maxHeight?: number;
  /** Lets the code be selected. @default true */
  isSelectable?: boolean;
  className?: string;
  'data-testid'?: string;
};

const signs: Record<LineKind, string> = { context: ' ', added: '+', removed: '-', header: ' ' };

/** How long "Copied" shows after a copy, in ms. */
const copiedFor = 1500;

/** The built-in copy button; "Copied" shows in a bubble above it, off the code, which the host closes after a moment. */
function CopyButton({ code, label, onCopy, testId }: { code: string; label: string; onCopy: (code: string) => void | Promise<unknown>; testId?: string }) {
  const id = useId();
  const copied = useLocal('copied', false);
  const copy = async () => {
    try {
      await onCopy(code);
      settleFlag(copied, true);
    } catch (error) {
      reportFailure('CodeBlock', label, error);
    }
  };
  return (
    <>
      <view id={`${id}-anchor`} className={r.copy.base}>
        <IconButton
          label={copied.value ? liuliText('@liuli.copied') : liuliText('@liuli.copy')}
          icon={copied.value ? 'check' : 'copy'}
          size="sm"
          onClick={copy}
          data-testid={testId && `${testId}-copy`}
        />
      </view>
      <view
        id={id}
        popover="manual"
        open={copied.value}
        closeAfter={copiedFor}
        onToggle={open => { if (!open) settleFlag(copied, false); }}
        anchor={`${id}-anchor`}
        role="status"
        style={{ positionArea: 'top' }}
        className={r.copied.base}
        data-testid={testId && `${testId}-copied`}
      >
        <text className={r.copiedText.base}>{liuliText('@liuli.copied')}</text>
      </view>
    </>
  );
}

/**
 * Lines of code as they were written: fixed-width and scrolling sideways, or wrapped at the block's width. With
 * numbers, a diff or marked lines, each line's tint is a row behind the code, one line high, so the code stays one
 * text to select.
 */
export function CodeBlock(props: CodeBlockProps) {
  const {
    code,
    label,
    title,
    language,
    onCopy,
    action,
    maxHeight,
    isSelectable = true,
    className,
    'data-testid': testId, ...rest
  } = props;
  const { hasLineNumbers = false, startLineNumber = 1, highlightLines, isDiff = false, isWrapped = false, ...attributes } = rest;
  const lines = code.split('\n');
  const first = Math.max(1, Math.floor(startLineNumber));
  const marked = new Set(highlightLines ?? []);
  // A change keeps its own tint under a mark: in some packs the accent's tint is close to the removal's.
  const isChange = (kind: LineKind) => kind === 'added' || kind === 'removed';
  const kinds: LineKind[] = isDiff ? diffKinds(lines) : lines.map(() => 'context');
  const isLined = hasLineNumbers || isDiff || lines.some((_, index) => marked.has(first + index));
  const text = cx(r.text.base, isWrapped && !isLined ? r.text.wrapped : r.text.unwrapped, isSelectable && r.text.selectable);
  // A diff's header lines are not code: they are left out of the grammar's reading and drawn quiet.
  const read = lines.map((line, index) => (kinds[index] === 'header' ? '' : isDiff ? line.slice(1) : line));
  const colored = linesOf(highlight(read.join('\n'), language), lines.length);
  const codeLines: Run[][] = lines.map((line, index) => {
    const end = index < lines.length - 1 ? '\n' : '';
    if (kinds[index] === 'header') return [{ text: line + end, isHeader: true }];
    return colored[index].map(run => ({ text: run.text, token: run.kind }));
  });
  const marks = (each: (kind: LineKind, index: number) => string) =>
    kinds.map((kind, index) => [{ text: `${each(kind, index)}${index < lines.length - 1 ? '\n' : ''}`, mark: kind }]);
  const body = isLined ? (
    <scroll className={r.scroll.lined}>
      <view className={r.lines.base}>
        <view aria-hidden className={r.tints.base}>
          {kinds.map((kind, index) => (
            <view key={`${index}`} className={cx(r.tint.base, marked.has(first + index) && !isChange(kind) ? r.tint.highlighted : r.tint[kind])} />
          ))}
        </view>
        {hasLineNumbers && codeColumn(r.gutter.base, marks((_, index) => `${first + index}`), true)}
        {isDiff && codeColumn(cx(r.signs.base, hasLineNumbers && r.signs.numbered), marks(kind => signs[kind]))}
        {codeColumn(cx(text, r.text.lined, isDiff ? r.text.afterSign : r.text.start), codeLines)}
      </view>
    </scroll>
  ) : isWrapped ? (
    <view className={r.wrap.base}>{codeColumn(text, codeLines)}</view>
  ) : (
    <scroll className={r.scroll.base}>
      {codeColumn(text, codeLines)}
    </scroll>
  );
  return (
    <view role="group" aria-label={label} data-testid={testId} {...dataOf(attributes)} className={cx(r.block.base, className)}>
      {(title !== undefined || action !== undefined || onCopy !== undefined) && (
        <view className={r.header.base}>
          <text className={r.title.base}>{title ?? ''}</text>
          {action}
          {onCopy && <CopyButton code={code} label={label} onCopy={onCopy} testId={testId} />}
        </view>
      )}
      {maxHeight === undefined ? body : (
        <scroll className={r.capped.base} style={{ maxHeight: Math.max(0, maxHeight) }}>{body}</scroll>
      )}
    </view>
  );
}
