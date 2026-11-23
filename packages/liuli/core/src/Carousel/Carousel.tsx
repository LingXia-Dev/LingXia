import { childrenToArray, cx, useBox, useLocal, type Child, type Local } from '@lingxia/view';
import { IconButton } from '../IconButton/IconButton';
import { liuliText } from '../i18n/text';
import { PageControl } from '../PageControl/PageControl';
import { carouselRecipe as r } from './Carousel.recipe';
import { dataOf } from '../internal/data';

export type CarouselDots = 'below' | 'over' | 'none';

/** Which page is in view: a View-local cell, or a value with its change handler, or neither (the carousel holds it). */
export type CarouselIndex =
  | {
      /** A View-local cell holding the page, from 0, to read or set it from outside. */
      bind?: Local<number>;
      index?: never;
      onIndexChange?: never;
    }
  | {
      bind?: never;
      /** The page in view, from 0, held by the caller: in Logic state, or to open at a deep link. */
      index: number;
      /** Runs with the page a swipe, a dot or a button moves to. */
      onIndexChange: (index: number) => void;
    };

export type CarouselProps = CarouselIndex & {
  /** Names the pages for assistive tech: "Photos of the house". */
  label: string;
  /** The pages, one child each; every page is as wide as the carousel, or `itemWidth`. */
  children: Child;
  /** The page it starts on when the carousel holds it. @default 0 */
  defaultIndex?: number;
  /** Each dot's accessible name, written by the app: `(page, count) => \`Photo ${page + 1} of ${count}\``. Defaults to "2 of 5" in the page's language. */
  pageLabel?: (page: number, count: number) => string;
  /** `over` lays the dots on the bottom edge of the pages, for pictures; `none` leaves paging to the swipe and the buttons. @default 'below' */
  dots?: CarouselDots;
  /** Each item's width in points: a rail of cards, several in view, that stops on each. Without it a page fills the carousel. */
  itemWidth?: number;
  /** Round buttons over the side edges that move a page; with a pointer, where a swipe is not to hand. @default false */
  hasButtons?: boolean;
  /** With the buttons: past the last page comes the first, and before the first the last. @default false */
  hasLoop?: boolean;
  /** The previous button's accessible name: "Previous photo". Needed with `hasButtons`. */
  previousLabel?: string;
  /** The next button's accessible name: "Next photo". Needed with `hasButtons`. */
  nextLabel?: string;
  className?: string;
  /** On the root; the dots get `<testid>-dots`, the buttons `<testid>-previous` and `<testid>-next`. */
  'data-testid'?: string;
};

// The rail's gap in points, as the recipe sets it.
const railGap = 12;

/** A few pages to swipe through, one at a time, with dots that follow; or a rail of cards. */
export function Carousel({
  label, children, bind, index, onIndexChange, defaultIndex = 0, pageLabel = (page: number, count: number) => liuliText('@liuli.itemOf', { page: page + 1, count }), dots = 'below', itemWidth, hasButtons = false, hasLoop = false, previousLabel, nextLabel, className, 'data-testid': testId, ...attributes
}: CarouselProps) {
  const local = useLocal('page', defaultIndex);
  const cell = bind ?? local;
  const page = onIndexChange !== undefined ? { value: index ?? 0, set: onIndexChange } : cell;
  const follow = onIndexChange !== undefined ? { snapIndex: index ?? 0, onSnapChange: onIndexChange } : { bind: cell };
  const dotsFollow = onIndexChange !== undefined ? { value: index ?? 0, onChange: onIndexChange } : { bind: cell };
  const pages = childrenToArray(children);
  const box = useBox('rail');
  const isRail = itemWidth !== undefined;
  // On a rail the last stops share one end; the furthest a page can go is the one that brings the end into view.
  const room = box.value?.width;
  const last = isRail && room !== undefined
    ? Math.min(pages.length - 1, Math.max(0, Math.ceil((pages.length * (itemWidth + railGap) - railGap - room) / (itemWidth + railGap))))
    : pages.length - 1;
  const at = Math.min(Math.max(0, Math.round(page.value)), last);
  const step = (direction: -1 | 1) => {
    const next = at + direction;
    page.set(next < 0 ? (hasLoop ? last : 0) : next > last ? (hasLoop ? 0 : last) : next);
  };
  const button = (direction: -1 | 1) => {
    const isShown = hasLoop ? last > 0 : direction < 0 ? at > 0 : at < last;
    return isShown && (
      <view className={cx(r.button.base, direction < 0 ? r.button.start : r.button.end)}>
        <view className={r.button.chip}>
          <IconButton
            label={(direction < 0 ? previousLabel : nextLabel) ?? ''}
            icon={direction < 0 ? 'chevron-start' : 'chevron-end'}
            variant="ghost"
            size="sm"
            onClick={() => step(direction)}
            data-testid={testId && `${testId}-${direction < 0 ? 'previous' : 'next'}`}
          />
        </view>
      </view>
    );
  };
  return (
    <view data-testid={testId} {...dataOf(attributes)} className={cx(r.root.base, className)}>
      <view className={r.frame.base}>
        <scroll box={box} {...follow} className={r.scroll.base}>
          <view role="list" aria-label={label} className={cx(r.track.base, isRail && r.track.rail)}>
            {pages.map((content, index) => (
              <view key={`page-${index}`} role="listitem" style={isRail ? { width: itemWidth } : undefined} className={cx(r.page.base, !isRail && r.page.full)}>
                {content}
              </view>
            ))}
          </view>
        </scroll>
        {hasButtons && button(-1)}
        {hasButtons && button(1)}
      </view>
      {dots !== 'none' && last > 0 && (
        <PageControl
          label={label}
          count={last + 1}
          {...dotsFollow}
          pageLabel={index => pageLabel(index, last + 1)}
          tone={dots === 'over' ? 'on-media' : 'accent'}
          className={dots === 'over' ? r.dots.over : undefined}
          data-testid={testId && `${testId}-dots`}
        />
      )}
    </view>
  );
}
