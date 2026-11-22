import { cx, useLocal, useView, type Child } from '@lingxia/view';
import { Button } from '../Button/Button';
import { splitViewRecipe as r } from './SplitView.recipe';

/** On a compact screen, the pane that shows. */
export type SplitViewPane = 'sidebar' | 'content';

/** `prose` keeps text to about 720 px, `wide` examples to about 960, `full` takes the pane. */
export type SplitViewContentWidth = 'prose' | 'wide' | 'full';

export interface SplitViewProps {
  /** The sidebar's content, usually a NavList; it scrolls on its own. */
  sidebar: Child;
  /** The content beside it; it scrolls on its own. */
  children: Child;
  /** Actions on what the content shows, such as Approve: a row under the content, outside its scroll, so they stay in view. */
  contentActions?: Child;
  /** Names the sidebar for assistive tech: what it lists, "Mailboxes", "Components". */
  sidebarLabel: string;
  /** Names the content: the open page's title. */
  contentLabel: string;
  /** On a compact screen one pane shows at a time: which. A regular screen shows both and ignores it. @default 'sidebar' */
  pane?: SplitViewPane;
  /** On a compact screen: the back button over the content was pressed; show the sidebar. */
  onBack?: () => void;
  /** The back button's label: where it leads. @default `sidebarLabel` */
  backLabel?: string;
  /** The sidebar's width in px, beside the content. @default 264 */
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
 * A sidebar beside its content on a regular screen, each pane scrolling on its own; on a compact
 * screen one pane at a time, the sidebar first and the content under a way back. It fills its page;
 * put it straight in a `<page>`, without a scroll around it.
 */
export function SplitView({
  sidebar, children, contentActions, sidebarLabel, contentLabel, pane = 'sidebar', onBack, backLabel, sidebarWidth = 264, contentKey, contentWidth = 'wide', className, 'data-testid': testId,
}: SplitViewProps) {
  const { env } = useView();
  const wentBack = useLocal('went-back', false);
  if (env.sizeClass === 'compact') {
    return (
      <view data-testid={testId} className={cx(r.root.base, r.root.compact, className)}>
        {pane === 'sidebar' ? (
          <view key="sidebar" role="group" aria-label={sidebarLabel} data-testid={testId && `${testId}-sidebar`} className={cx(r.pane.base, r.pane.sidebar, wentBack.value && r.pane.returning)}>
            <scroll className={r.scroll.base}>
              <view className={cx(r.sidebarInner.base, r.sidebarInner.compact)}>{sidebar}</view>
            </scroll>
          </view>
        ) : (
          <view key="content" role="group" aria-label={contentLabel} data-testid={testId && `${testId}-content`} className={cx(r.pane.base, r.pane.content)}>
            <view className={r.back.base}>
              <Button label={backLabel ?? sidebarLabel} startIcon="chevron-start" variant="ghost" size="sm" width="content" className={r.back.button} onClick={() => { wentBack.set(true); onBack?.(); }} data-testid={testId && `${testId}-back`} />
            </view>
            <scroll key={contentKey} className={r.scroll.base}>
              <view className={cx(r.contentInner.base, r.contentInner.compact)}>{children}</view>
            </scroll>
            {contentActions !== undefined && <view className={cx(r.actions.base, r.actions.compact)} data-testid={testId && `${testId}-actions`}>{contentActions}</view>}
          </view>
        )}
      </view>
    );
  }
  return (
    <view data-testid={testId} className={cx(r.root.base, r.root.regular, className)}>
      <view role="group" aria-label={sidebarLabel} data-testid={testId && `${testId}-sidebar`} className={r.sidebar.base} style={{ width: sidebarWidth }}>
        <scroll className={r.scroll.base}>
          <view className={cx(r.sidebarInner.base, r.sidebarInner.regular)}>{sidebar}</view>
        </scroll>
      </view>
      <view role="group" aria-label={contentLabel} data-testid={testId && `${testId}-content`} className={r.content.base}>
        <scroll key={contentKey} className={r.scroll.base}>
          <view className={cx(r.contentInner.base, r.contentInner.regular, r.contentInner.width[contentWidth])}>{children}</view>
        </scroll>
        {contentActions !== undefined && <view className={cx(r.actions.base, r.actions.regular)} data-testid={testId && `${testId}-actions`}>{contentActions}</view>}
      </view>
    </view>
  );
}
