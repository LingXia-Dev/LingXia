import { cx, type Child } from '@lingxia/view';
import { topBarRecipe as r } from './TopBar.recipe';

export interface TopBarProps {
  /** Optional leading control (e.g. back button or filter). */
  leading?: Child;
  /** Centered title when no custom center children are provided. */
  title?: string;
  /** Custom center content (e.g. a SearchField). */
  children?: Child;
  /** Optional trailing page actions. */
  trailing?: Child;
  className?: string;
  'data-testid'?: string;
}

/**
 * Custom page top bar. The page already keeps its content clear of system bars and host
 * chrome, so the bar adds only its own padding; page code never measures chrome rects.
 */
export function TopBar({
  leading,
  title,
  children,
  trailing,
  className,
  'data-testid': testId,
}: TopBarProps) {
  return (
    <view data-testid={testId} className={cx(r.root.base, className)}>
      {leading !== undefined && <view className={r.leading.base}>{leading}</view>}
      <view className={r.center.base}>
        {children !== undefined ? children : title !== undefined ? <text className={r.title.base}>{title}</text> : null}
      </view>
      {trailing !== undefined && <view className={r.trailing.base}>{trailing}</view>}
    </view>
  );
}
