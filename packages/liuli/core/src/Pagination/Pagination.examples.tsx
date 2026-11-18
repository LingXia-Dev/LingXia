import { useLocal } from '@lingxia/view';
import { unused } from '../example';
import { Pagination } from './Pagination';

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

/** The home tile. */
export function Specimen() {
  return <Pagination label="Pages" variant="compact" value={3} pageCount={12} onChange={unused} {...words} />;
}
