import { cx, useView, type Child } from '@lingxia/view';
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
 * content clear of system bars; a host capsule floats over the bar's row, so the bar keeps its
 * content clear of it.
 */
export function TopBar({ heading, startContent, centerContent, endContent, className, 'data-testid': testId, ...attributes }: TopBarProps) {
  // At large text the centre takes its own row under the title and the actions.
  const isStacked = useLargeText() && centerContent !== undefined;
  const capsuleClear = useCapsuleClearance();
  const center = centerContent !== undefined && <view className={isStacked ? r.center.stacked : r.center.base}>{centerContent}</view>;
  return (
    <view data-testid={testId} {...dataOf(attributes)} className={cx(r.root.base, className)} style={capsuleClear}>
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

/**
 * The capsule is a local occlusion, not an inset: pad the side it floats on by its width plus a gap.
 * Padding, not a spacer, so a wrap can never leave the actions under it.
 */
function useCapsuleClearance(): { paddingLeft: number } | { paddingRight: number } | undefined {
  const { env } = useView();
  const capsule = env.capsule;
  if (!capsule) return undefined;
  const width = env.viewport.width;
  return capsule.x > width / 2 ? { paddingRight: width - capsule.x + 8 } : { paddingLeft: capsule.x + capsule.width + 8 };
}
