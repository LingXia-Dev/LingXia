import { cx, type RunChild } from '@lingxia/view';
import { inlineKind } from '../schema/helpers';
import type { Inline, InlineAtom, InlineMath, InlineRun } from '../schema/types';
import { formatInlineLatex } from './MathView';
import { jinshuViewRecipe as r } from '../JinshuView/JinshuView.recipe';

export type InlineAtomRenderer = (props: { readonly atom: InlineAtom }) => RunChild;

export interface InlineRunsOptions {
  readonly blockId: string;
  readonly onLinkPress?: (info: { readonly blockId: string; readonly href: string }) => void;
  readonly onInPageLink?: (targetBlockId: string) => void;
  readonly customInlines?: Readonly<Record<string, InlineAtomRenderer>>;
}

function runMarksClass(run: InlineRun): string {
  const hl = run.highlight ? r.run.highlight[run.highlight] : false;
  return cx(
    run.bold && r.run.bold,
    run.italic && r.run.italic,
    run.underline && r.run.underline,
    run.strike && r.run.strike,
    run.code && r.run.code,
    run.script === 'sub' && r.run.sub,
    run.script === 'super' && r.run.super,
    hl,
  );
}

/**
 * Renders `Inline[]` into `<text>` children (`RunChild[]`), grouping consecutive
 * runs sharing the same `link` href into a single `<link>` node.
 */
export function renderInlineRuns(inlines: readonly Inline[], opts: InlineRunsOptions): RunChild[] {
  const out: RunChild[] = [];
  // Respect LingXia's <= 256 runs per <text> hard limit
  const safeSlice = inlines.length > 240 ? inlines.slice(0, 240) : inlines;

  for (let i = 0; i < safeSlice.length; ) {
    const item = safeSlice[i];
    const kind = inlineKind(item);

    if (kind === 'break') {
      out.push('\n');
      i += 1;
      continue;
    }

    if (kind === 'math') {
      const math = item as InlineMath;
      out.push(
        <span key={`math-${i}`} className={r.run.math}>
          {formatInlineLatex(math.math)}
        </span>,
      );
      i += 1;
      continue;
    }

    if (kind === 'atom') {
      const atom = item as InlineAtom;
      const custom = opts.customInlines?.[atom.inline];
      if (custom) {
        out.push(custom({ atom }));
      } else {
        out.push(
          <span key={`atom-${i}`} className={r.run.atom}>
            {atom.fallback}
          </span>,
        );
      }
      i += 1;
      continue;
    }

    if (kind === 'text') {
      const href = (item as InlineRun).link;
      let end = i + 1;
      while (href !== undefined && end < safeSlice.length) {
        const next = safeSlice[end];
        if (inlineKind(next) === 'text' && (next as InlineRun).link === href) end += 1;
        else break;
      }

      const spans = safeSlice.slice(i, end).map((sub, idx) => {
        const subRun = sub as InlineRun;
        const cls = runMarksClass(subRun);
        return cls === '' ? (
          subRun.text
        ) : (
          <span key={`r-${i + idx}`} className={cls}>
            {subRun.text}
          </span>
        );
      });

      if (href !== undefined) {
        const handleClick = () => {
          if (href.startsWith('#')) {
            opts.onInPageLink?.(href.slice(1));
          } else {
            opts.onLinkPress?.({ blockId: opts.blockId, href });
          }
        };
        out.push(
          <link key={`link-${i}`} onClick={handleClick} className={r.run.link}>
            {spans}
          </link>,
        );
      } else {
        out.push(...spans);
      }
      i = end;
      continue;
    }

    i += 1;
  }

  if (inlines.length > 240) {
    out.push('…');
  }

  return out;
}
