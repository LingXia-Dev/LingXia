import { childrenToArray, cx, useLocal, type Child, type Local } from '@lingxia/view';
import { PageControl } from '../PageControl/PageControl';
import { carouselRecipe as r } from './Carousel.recipe';

export type CarouselDots = 'below' | 'over' | 'none';

export interface CarouselProps {
  /** Names the pages for assistive tech: "Photos of the house". */
  label: string;
  /** The pages, one child each; every page is as wide as the carousel. */
  children: Child;
  /** A View-local cell holding the page, from 0, to read or set it from outside. */
  bind?: Local<number>;
  /** Each dot's accessible name, written by the app: `(page, count) => \`Photo ${page + 1} of ${count}\``. */
  pageLabel: (page: number, count: number) => string;
  /** `over` lays the dots on the bottom edge of the pages, for pictures; `none` leaves paging to the swipe. @default 'below' */
  dots?: CarouselDots;
  className?: string;
  /** On the root; the dots get `<testid>-dots`. */
  'data-testid'?: string;
}

/** A few pages to swipe through, one at a time, with dots that follow. */
export function Carousel({ label, children, bind, pageLabel, dots = 'below', className, 'data-testid': testId }: CarouselProps) {
  const local = useLocal('page', 0);
  const page = bind ?? local;
  const pages = childrenToArray(children);
  return (
    <view data-testid={testId} className={cx(r.root.base, className)}>
      <scroll bind={page} className={r.scroll.base}>
        <view role="list" aria-label={label} className={r.track.base}>
          {pages.map((content, index) => <view key={`page-${index}`} role="listitem" className={r.page.base}>{content}</view>)}
        </view>
      </scroll>
      {dots !== 'none' && pages.length > 1 && (
        <PageControl
          label={label}
          count={pages.length}
          bind={page}
          pageLabel={at => pageLabel(at, pages.length)}
          tone={dots === 'over' ? 'on-media' : 'accent'}
          className={dots === 'over' ? r.dots.over : undefined}
          data-testid={testId && `${testId}-dots`}
        />
      )}
    </view>
  );
}
