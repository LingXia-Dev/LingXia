import { cx, useBox, useView, type Local } from '@lingxia/view';
import { liuliText } from '../i18n/text';
import { IconButton } from '../IconButton/IconButton';
import { Select } from '../Select/Select';
import { Glyph } from '../internal/Glyph';
import { paginationRecipe as r } from './Pagination.recipe';
import { dataOf } from '../internal/data';

export type PaginationBinding =
  | {
      /** A View-local cell the control reads and writes; use it or `value` with `onChange`. */
      bind: Local<number>;
      value?: never;
      onChange?: never;
    }
  | {
      bind?: never;
      /** The current page, from 1. */
      value: number;
      /** Runs with the page chosen. */
      onChange: (page: number) => void;
    };

export type PaginationProps = PaginationBinding & {
  /** Names the control for assistive tech: "Pages", "Results pages". */
  label: string;
  /** How many pages there are. Without it or `totalItems` the count is unknown and `hasMore` says whether there is a next page. */
  pageCount?: number;
  /** How many items there are, when the pages are counted from it and `pageSize`. */
  totalItems?: number;
  /** With no count, as with a cursor: whether a page follows this one. @default false */
  hasMore?: boolean;
  /** A page button's accessible name: `page => \`Page ${page}\``. Defaults to "Page 2" in the page's language. */
  pageLabel?: (page: number) => string;
  previousLabel?: string;
  nextLabel?: string;
  /** How many pages to show on each side of the current one. @default 1 */
  siblings?: number;
  /** `compact` shows the two arrows around "3 / 12"; `auto` uses it where the pointer is a finger. Page numbers that do not fit their row give way to it. @default 'auto' */
  variant?: 'auto' | 'pages' | 'compact';
  /** The text between the arrows, read as the position: with `compact`, and wherever the count is unknown. By default "3 / 12", or "3". */
  positionLabel?: (page: number, pageCount: number | undefined) => string;
  /** Items to a page, with `totalItems` and the page-size choice. @default 20 */
  pageSize?: number;
  /** The page sizes to choose between; with them a choice shows before the arrows. */
  pageSizes?: readonly number[];
  /** Runs with the size chosen, then `onChange` with the page that keeps the first item of this one in view. */
  onPageSizeChange?: (pageSize: number) => void;
  /** The page-size choice's accessible name: "Rows per page". Defaults to "Items per page" in the page's language. */
  pageSizeLabel?: string;
  className?: string;
  /** On the control; a page is `<id>-<n>`, the arrows `<id>-previous` and `<id>-next`. */
  'data-testid'?: string;
};

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
  label, bind, value: own, pageCount: counted, totalItems, hasMore = false, onChange: ownChange, pageLabel = (page: number) => liuliText('@liuli.page', { page }), previousLabel = liuliText('@liuli.previousPage'), nextLabel = liuliText('@liuli.nextPage'), siblings = 1, variant = 'auto', positionLabel,
  pageSize = 20, pageSizes, onPageSizeChange, pageSizeLabel = liuliText('@liuli.pageSize'), className, 'data-testid': testId, ...attributes
}: PaginationProps) {
  const page = bind ? bind.value : own;
  const onChange = (next: number) => bind ? bind.set(next) : ownChange?.(next);
  // There is always a first page, even of nothing.
  const pageCount = counted !== undefined ? Math.max(1, Math.floor(counted) || 1) : totalItems === undefined ? undefined : Math.max(1, Math.ceil(totalItems / Math.max(1, pageSize)));
  const { env } = useView();
  // A row of page numbers needs a pointer's precision and room; a finger gets the arrows and the position.
  // With no count there are no page numbers to show, only where the reader is.
  const wantsCompact = pageCount === undefined || variant === 'compact' || (variant === 'auto' && env.pointer === 'coarse');
  const current = pageCount === undefined ? Math.max(1, page) : Math.min(Math.max(1, page), Math.max(1, pageCount));
  const last = pageCount ?? current;
  // The numbers are shown only where they fit: first with fewer neighbours, then as the position.
  const box = useBox('pager');
  const room = box.value?.width ?? Infinity;
  const fits = (around: number) => arrows + pagesShown(current, last, around).reduce<number>((sum, each) => sum + (each === null ? gapWidth : pageWidth) + between, 0) <= room;
  const around = fits(siblings) ? siblings : 0;
  const isCompact = wantsCompact || !fits(around);
  const arrow = (direction: -1 | 1) => (
    <IconButton
      label={direction < 0 ? previousLabel : nextLabel}
      icon={direction < 0 ? 'chevron-start' : 'chevron-end'}
      variant="ghost"
      isDisabled={direction < 0 ? current <= 1 : pageCount === undefined ? !hasMore : current >= pageCount}
      onClick={() => onChange(current + direction)}
      data-testid={testId && `${testId}-${direction < 0 ? 'previous' : 'next'}`}
    />
  );
  const position = positionLabel ? positionLabel(current, pageCount) : pageCount === undefined ? `${current}` : `${current} / ${pageCount}`;
  const controls = (
    <>
      {arrow(-1)}
      {isCompact
        ? <text role="status" className={r.position.base}>{position}</text>
        : (
          <view className={r.pages.base}>
            {pagesShown(current, last, around).map((each, index) => (each === null
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
    </>
  );
  if (!pageSizes?.length) {
    return <view box={box} role="group" aria-label={label} data-testid={testId} {...dataOf(attributes)} className={cx(r.root.base, className)}>{controls}</view>;
  }
  // A new size keeps the first item of this page in view.
  const resize = (next: string | null) => {
    const size = Number(next);
    if (!size || size === pageSize) return;
    onPageSizeChange?.(size);
    onChange(Math.floor(((current - 1) * pageSize) / size) + 1);
  };
  return (
    <view role="group" aria-label={label} data-testid={testId} {...dataOf(attributes)} className={cx(r.sized.base, className)}>
      <Select
        label={pageSizeLabel}
        isLabelHidden
        size="sm"
        items={pageSizes.map(size => ({ value: String(size), label: String(size) }))}
        value={String(pageSize)}
        onChange={resize}
        className={r.size.base}
        data-testid={testId && `${testId}-size`}
      />
      <view box={box} className={r.controls.base}>{controls}</view>
    </view>
  );
}
