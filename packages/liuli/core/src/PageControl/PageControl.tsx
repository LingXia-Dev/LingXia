import { cx, type Local } from '@lingxia/view';
import { pageControlRecipe as r } from './PageControl.recipe';

/** `on-media` for dots laid over a picture. */
export type PageControlTone = 'accent' | 'on-media';

export type PageControlBinding =
  | {
      /** A View-local cell holding the page, from 0: the same cell a paging `<scroll>` binds, so the two follow each other. */
      bind: Local<number>;
      value?: never;
      onChange?: never;
    }
  | {
      bind?: never;
      /** The current page, from 0. */
      value: number;
      /** Runs with the page of the dot that was pressed. */
      onChange: (page: number) => void;
    };

export type PageControlProps = PageControlBinding & {
  /** Names the dots for assistive tech: "Photos". */
  label: string;
  /** How many pages there are. Past ten or so, show a count in words. */
  count: number;
  /** Each dot's accessible name, written by the app: `page => \`Photo ${page + 1}\``. */
  pageLabel: (page: number) => string;
  /** @default 'accent' */
  tone?: PageControlTone;
  className?: string;
  /** On the row; each dot gets `<testid>-<page>`. */
  'data-testid'?: string;
};

/** Dots that show which page of a few is in view, and go to one when pressed. */
export function PageControl(props: PageControlProps) {
  const { label, count, pageLabel, tone = 'accent', className, 'data-testid': testId } = props;
  const raw = props.bind ? props.bind.value : props.value;
  // A bound scroll reports fractions while it moves; the dot follows the nearest page.
  const at = Math.min(Math.max(0, Math.round(raw)), Math.max(0, count - 1));
  const go = (page: number) => (props.bind ? props.bind.set(page) : props.onChange(page));
  return (
    <view role="tablist" aria-label={label} data-testid={testId} className={cx(r.root.base, className)}>
      {Array.from({ length: count }, (_, page) => (
        <button
          key={`page-${page}`}
          role="tab"
          aria-label={pageLabel(page)}
          aria-selected={page === at}
          onClick={() => go(page)}
          data-testid={testId && `${testId}-${page}`}
          className={r.dot.base}
        >
          <view className={cx(r.mark.base, r.mark.state[page === at ? 'on' : 'off'], r.mark.tone[tone][page === at ? 'on' : 'off'])} />
        </button>
      ))}
    </view>
  );
}
