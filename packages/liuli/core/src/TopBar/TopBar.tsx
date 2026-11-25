import { cx, type Child } from '@lingxia/view';
import { dataOf } from '../internal/data';
import { useLargeText } from '../internal/largeText';
import { topBarRecipe as r } from './TopBar.recipe';

export interface TopBarProps {
  /** The page's title: a string is drawn as its heading; a node is drawn as given. It follows `startContent`. */
  heading?: Child;
  /** At the start, before the heading: a way back, a menu. */
  startContent?: Child;
  /** At the bar's middle while everything fits: a SearchField, a segmented control. */
  centerContent?: Child;
  /** At the end: the page's actions, a row of IconButtons. */
  endContent?: Child;
  className?: string;
  'data-testid'?: string;
}

/**
 * A page's own header row, for a page that draws no host navigation bar. The page already keeps its
 * content clear of system bars and host chrome, so the bar adds only its own padding.
 */
export function TopBar({ heading, startContent, centerContent, endContent, className, 'data-testid': testId, ...attributes }: TopBarProps) {
  // At large text the centre takes its own row under the title and the actions.
  const isStacked = useLargeText() && centerContent !== undefined;
  const center = centerContent !== undefined && <view className={isStacked ? r.center.stacked : r.center.base}>{centerContent}</view>;
  return (
    <view data-testid={testId} {...dataOf(attributes)} className={cx(r.root.base, className)}>
      <view className={r.start.base}>
        {startContent}
        {typeof heading === 'string' ? <text role="heading" headingLevel={1} className={r.heading.base}>{heading}</text> : heading}
      </view>
      {!isStacked && center}
      <view className={r.end.base}>{endContent}</view>
      {isStacked && center}
    </view>
  );
}
