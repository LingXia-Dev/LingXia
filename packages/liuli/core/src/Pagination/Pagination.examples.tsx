import { useLocal } from '@lingxia/view';
import { unused } from '../example';
import { Pagination, type PaginationProps } from './Pagination';

const words = { pageLabel: (page: number) => `Page ${page}`, previousLabel: 'Previous page', nextLabel: 'Next page' };

/** Twelve pages: a finger gets the arrows and the position, a mouse the page numbers. */
export function Results() {
  const page = useLocal('page', 3);
  return (
    <view className="gap-2">
      <Pagination label="Results pages" value={page.value} pageCount={12} onChange={page.set} positionLabel={(at, of) => `Page ${at} of ${of}`} {...words} data-testid="results" />
      <text role="status" className="text-footnote text-muted tabular-nums">{`Showing ${(page.value - 1) * 20 + 1}–${page.value * 20} of 240`}</text>
    </view>
  );
}

/** The page numbers, whatever the pointer: first, last, the current page and its neighbours. */
export function Pages() {
  const page = useLocal('page', 6);
  return <Pagination label="Pages" variant="pages" value={page.value} pageCount={12} onChange={page.set} {...words} data-testid="pages" />;
}

/** The arrows and the position, whatever the pointer. */
export function Compact() {
  const page = useLocal('page', 1);
  return <Pagination label="Pages" variant="compact" value={page.value} pageCount={4} onChange={page.set} {...words} data-testid="compact" />;
}

/** Page numbers in a row too narrow for them: the neighbours go first, then the numbers give way to the position. */
export function Narrow() {
  const page = useLocal('page', 6);
  return (
    <view className="gap-3">
      <view className="w-[280px]"><Pagination label="Pages" variant="pages" value={page.value} pageCount={12} onChange={page.set} {...words} data-testid="narrow" /></view>
      <view className="w-[232px]"><Pagination label="Pages" variant="pages" value={page.value} pageCount={12} onChange={page.set} {...words} data-testid="narrower" /></view>
    </view>
  );
}

/** A feed read by cursor: no count, so the position and the arrows; Next stops where nothing follows. */
export function Cursor() {
  const page = useLocal('page', 1);
  return (
    <Pagination
      label="Activity pages"
      value={page.value}
      hasMore={page.value < 3}
      onChange={page.set}
      positionLabel={at => `Page ${at}`}
      {...words}
      data-testid="cursor"
    />
  );
}

/** The reader picks how many rows a page holds; a new size keeps the first row of this page in view. */
export function PageSize() {
  const page = useLocal('page', 3);
  const size = useLocal('size', 20);
  const first = (page.value - 1) * size.value + 1;
  return (
    <view className="gap-2">
      <Pagination
        label="Invoice pages"
        value={page.value}
        totalItems={240}
        pageSize={size.value}
        pageSizes={[10, 20, 50]}
        pageSizeLabel="Rows per page"
        onPageSizeChange={size.set}
        onChange={page.set}
        {...words}
        data-testid="sized"
      />
      <text role="status" className="text-footnote text-muted tabular-nums" data-testid="sized-range">{`Showing ${first}–${Math.min(240, first + size.value - 1)} of 240`}</text>
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return <Pagination label="Pages" variant="compact" value={3} pageCount={12} onChange={unused} {...words} />;
}

/** The playground's props that are not plain values. */
export function playground() {
  return {
    pageLabel: words.pageLabel,
    positionLabel: (at: number, of: number | undefined) => `Page ${at} of ${of}`,
  } satisfies Partial<PaginationProps>;
}
