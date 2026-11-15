import { cx, useBox, useView } from '@lingxia/view';
import { IconButton } from '../IconButton/IconButton';
import { Glyph } from '../internal/Glyph';
import { paginationRecipe as r } from './Pagination.recipe';

export interface PaginationProps {
  /** Names the control for assistive tech: "Pages", "Results pages". */
  label: string;
  /** The current page, from 1. */
  page: number;
  /** How many pages there are. */
  pageCount: number;
  /** Runs with the page chosen. */
  onChange: (page: number) => void;
  /** A page button's accessible name: `page => \`Page ${page}\``. */
  pageLabel: (page: number) => string;
  previousLabel: string;
  nextLabel: string;
  /** How many pages to show on each side of the current one. @default 1 */
  siblings?: number;
  /** `compact` shows the two arrows around "3 / 12"; `auto` uses it where the pointer is a finger. Page numbers that do not fit their row give way to it. @default 'auto' */
  variant?: 'auto' | 'pages' | 'compact';
  /** With `compact`: the text between the arrows, read as the position. By default "3 / 12". */
  positionLabel?: (page: number, pageCount: number) => string;
  className?: string;
  /** On the control; a page is `<id>-<n>`, the arrows `<id>-previous` and `<id>-next`. */
  'data-testid'?: string;
}

/** The pages to show: the first, the last, the current one with its neighbours, and `null` where pages are left out. */
export function pagesShown(page: number, pageCount: number, siblings: number): (number | null)[] {
  const wanted = new Set([1, pageCount]);
  for (let each = page - siblings; each <= page + siblings; each++) if (each >= 1 && each <= pageCount) wanted.add(each);
  // A gap of one page is that page, not an ellipsis.
  for (const each of [...wanted]) { if (wanted.has(each + 2)) wanted.add(each + 1); }
  const sorted = [...wanted].sort((a, b) => a - b);
  return sorted.flatMap((each, index) => (index > 0 && each - sorted[index - 1] > 1 ? [null, each] : [each]));
}

// Widths in points, as the recipe sets them: the two arrows with their gaps, a page, an ellipsis, the gap between.
const [arrows, pageWidth, gapWidth, between] = [88, 40, 24, 4];

/** Moves through a long set a page at a time. */
export function Pagination({
  label, page, pageCount, onChange, pageLabel, previousLabel, nextLabel, siblings = 1, variant = 'auto', positionLabel, className, 'data-testid': testId,
}: PaginationProps) {
  const { env } = useView();
  // A row of page numbers needs a pointer's precision and room; a finger gets the arrows and the position.
  const wantsCompact = variant === 'compact' || (variant === 'auto' && env.pointer === 'coarse');
  const current = Math.min(Math.max(1, page), Math.max(1, pageCount));
  // The numbers are shown only where they fit: first with fewer neighbours, then as the position.
  const box = useBox('pager');
  const room = box.value?.width ?? Infinity;
  const fits = (around: number) => arrows + pagesShown(current, pageCount, around).reduce<number>((sum, each) => sum + (each === null ? gapWidth : pageWidth) + between, 0) <= room;
  const around = fits(siblings) ? siblings : 0;
  const isCompact = wantsCompact || !fits(around);
  const arrow = (direction: -1 | 1) => (
    <IconButton
      label={direction < 0 ? previousLabel : nextLabel}
      icon={direction < 0 ? 'chevron-start' : 'chevron-end'}
      variant="ghost"
      isDisabled={direction < 0 ? current <= 1 : current >= pageCount}
      onClick={() => onChange(current + direction)}
      data-testid={testId && `${testId}-${direction < 0 ? 'previous' : 'next'}`}
    />
  );
  return (
    <view box={box} role="group" aria-label={label} data-testid={testId} className={cx(r.root.base, className)}>
      {arrow(-1)}
      {isCompact
        ? <text role="status" className={r.position.base}>{positionLabel ? positionLabel(current, pageCount) : `${current} / ${pageCount}`}</text>
        : (
          <view className={r.pages.base}>
            {pagesShown(current, pageCount, around).map((each, index) => (each === null
              ? <view key={`gap-${index}`} aria-hidden className={r.gap.base}><Glyph name="more" className={r.gap.icon} /></view>
              : (
                <button
                  key={String(each)}
                  aria-label={pageLabel(each)}
                  aria-pressed={each === current}
                  onClick={() => { if (each !== current) onChange(each); }}
                  data-testid={testId && `${testId}-${each}`}
                  className={cx(r.page.base, each === current ? r.page.current : r.page.idle)}
                >
                  <text className={cx(r.pageLabel.base, each === current && r.pageLabel.current)}>{String(each)}</text>
                </button>
              )))}
          </view>
        )}
      {arrow(1)}
    </view>
  );
}
