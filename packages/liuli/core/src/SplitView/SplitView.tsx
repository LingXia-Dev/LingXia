import { cx, type Child } from '@lingxia/view';
import { splitViewRecipe as r } from './SplitView.recipe';

/** `prose` keeps text to about 720 px, `wide` examples to about 960, `full` takes the pane. */
export type SplitViewContentWidth = 'prose' | 'wide' | 'full';

export interface SplitViewProps {
  /** The sidebar's content, usually a NavList; it scrolls on its own. */
  sidebar: Child;
  /** The content beside it; it scrolls on its own. */
  children: Child;
  /** Names the sidebar for assistive tech: "Components". */
  sidebarLabel: string;
  /** Names the content: the open page's title. */
  contentLabel: string;
  /** The sidebar's width in px. @default 264 */
  sidebarWidth?: number;
  /** Names what the content shows; a new key starts the content pane at its top, as opening a page does. */
  contentKey?: string;
  /** How wide the content may grow; past it the surplus falls on the far side. @default 'wide' */
  contentWidth?: SplitViewContentWidth;
  className?: string;
  /** On the root; the panes get `<testid>-sidebar` and `<testid>-content`. */
  'data-testid'?: string;
}

/**
 * A sidebar beside its content, for a wide window: each pane scrolls on its own. It fills its page;
 * put it straight in a `<page>`, without a scroll around it.
 */
export function SplitView({ sidebar, children, sidebarLabel, contentLabel, sidebarWidth = 264, contentKey, contentWidth = 'wide', className, 'data-testid': testId }: SplitViewProps) {
  return (
    <view data-testid={testId} className={cx(r.root.base, className)}>
      <view role="group" aria-label={sidebarLabel} data-testid={testId && `${testId}-sidebar`} className={r.sidebar.base} style={{ width: sidebarWidth }}>
        <scroll className={r.scroll.base}>
          <view className={r.sidebarInner.base}>{sidebar}</view>
        </scroll>
      </view>
      <view role="group" aria-label={contentLabel} data-testid={testId && `${testId}-content`} className={r.content.base}>
        <scroll key={contentKey} className={r.scroll.base}>
          <view className={cx(r.contentInner.base, r.contentInner.width[contentWidth])}>{children}</view>
        </scroll>
      </view>
    </view>
  );
}
