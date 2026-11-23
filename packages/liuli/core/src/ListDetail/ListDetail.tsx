import {
  SizeClassScope,
  createContext,
  cx,
  useBackHandler,
  useContext,
  useLocal,
  useView,
  type Child,
  type Local,
} from '@lingxia/view';
import { Button } from '../Button/Button';
import { dataOf } from '../internal/data';
import { contentHinge } from '../internal/hinge';
import { listDetailRecipe as r } from './ListDetail.recipe';

/** `prose` keeps text to about 720 pt, `wide` examples to about 960, `full` takes the pane. */
export type ListDetailContentWidth = 'prose' | 'wide' | 'full';

interface ListDetailState {
  isCompact: boolean;
  selected: string | null;
  clearSelection: () => void;
  canGoBack: boolean;
  wentBack: boolean;
  listLabel: string;
  listWidth: number;
  hingeGapWidth: number;
  contentWidth: ListDetailContentWidth;
  testId?: string;
}

const ListDetailContext = createContext<ListDetailState | null>(null);

export interface ListDetailListProps {
  /** The list, usually an ItemList of rows; it scrolls on its own. */
  children?: Child;
  className?: string;
}

export interface ListDetailDetailProps {
  /** The chosen record's title: on a phone, the heading beside the back button; everywhere, the detail pane's name. */
  title?: string;
  /** Shown beside the list while nothing is chosen, on a wide screen: what choosing a row will show. */
  placeholder?: Child;
  /** Actions on the chosen record, such as Approve: a row under the detail, outside its scroll, so they stay in view. */
  actions?: Child;
  /** The chosen record's detail; it scrolls on its own and starts at its top for each new choice. */
  children?: Child;
  className?: string;
}

/** Which row is chosen: a View-local cell, or a value with its change handler. The rows set it; Back clears it. */
export type ListDetailSelection =
  | {
      /** A View-local cell holding the chosen row's id, `null` while none is. */
      bind: Local<string | null>;
      value?: never;
      onChange?: never;
    }
  | {
      bind?: never;
      /** The chosen row's id, held by the caller: in Logic state, or from a deep link. `null` while none is. */
      value: string | null;
      /** Runs with `null` when Back or the back button leaves the detail on a phone. */
      onChange: (value: string | null) => void;
    };

export type ListDetailProps = ListDetailSelection & {
  /** Names the list pane: what it lists, "Orders". It is also the back button's label on a phone. */
  listLabel: string;
  /** The list's width in pt beside the detail, when no fold crosses the page. @default 360 */
  listWidth?: number;
  /** How wide the detail may grow; past it the surplus falls on the far side. @default 'wide' */
  contentWidth?: ListDetailContentWidth;
  /** `<ListDetail.List>` and `<ListDetail.Detail>`. */
  children?: Child;
  className?: string;
  /** On the root; the panes get `<testid>-list` and `<testid>-detail`, the back button `<testid>-back`, the actions `<testid>-actions`. */
  'data-testid'?: string;
};

/** The list pane of a `<ListDetail>`. */
export function ListDetailList({ children, className }: ListDetailListProps) {
  const ctx = useContext(ListDetailContext);
  if (!ctx) throw Error('ListDetail.List must be rendered inside <ListDetail>');
  const pane = (
    <view
      role="group"
      aria-label={ctx.listLabel}
      data-testid={ctx.testId && `${ctx.testId}-list`}
      className={cx(
        r.list.base,
        ctx.isCompact ? r.list.compact : r.list.regular,
        ctx.isCompact && ctx.selected !== null && r.list.hidden,
        ctx.isCompact && ctx.wentBack && r.list.returning,
        className,
      )}
      style={ctx.isCompact ? undefined : { width: ctx.listWidth }}
    >
      <scroll className={r.scroll.base}>
        <view className={cx(r.listInner.base, ctx.isCompact ? r.listInner.compact : r.listInner.regular)}>{children}</view>
      </scroll>
    </view>
  );
  return ctx.isCompact ? pane : <SizeClassScope value="compact">{pane}</SizeClassScope>;
}

/** The detail pane of a `<ListDetail>`. */
export function ListDetailDetail({ title, placeholder, actions, children, className }: ListDetailDetailProps) {
  const ctx = useContext(ListDetailContext);
  if (!ctx) throw Error('ListDetail.Detail must be rendered inside <ListDetail>');
  const hasSelection = ctx.selected !== null;
  if (ctx.isCompact && !hasSelection) return null;
  const actionRow = hasSelection && actions !== undefined && (
    <view className={cx(r.actions.base, ctx.isCompact ? r.actions.compact : r.actions.regular)} data-testid={ctx.testId && `${ctx.testId}-actions`}>{actions}</view>
  );
  if (ctx.isCompact) {
    return (
      <view role="group" aria-label={title ?? ctx.listLabel} data-testid={ctx.testId && `${ctx.testId}-detail`} className={cx(r.detail.base, r.detail.compact, className)}>
        <view className={r.back.base}>
          {ctx.canGoBack && (
            <Button label={ctx.listLabel} startIcon="chevron-start" variant="ghost" size="sm" width="content" className={r.back.button} onClick={ctx.clearSelection} data-testid={ctx.testId && `${ctx.testId}-back`} />
          )}
          {title !== undefined && <text role="heading" headingLevel={1} className={r.title.base}>{title}</text>}
        </view>
        <scroll key={ctx.selected ?? ''} className={r.scroll.base}>
          <view className={cx(r.detailInner.base, r.detailInner.compact)}>{children}</view>
        </scroll>
        {actionRow}
      </view>
    );
  }
  return (
    <SizeClassScope value="compact">
      <view
        role="group"
        aria-label={title ?? ctx.listLabel}
        data-testid={ctx.testId && `${ctx.testId}-detail`}
        className={cx(r.detail.base, className)}
        style={ctx.hingeGapWidth > 0 ? { marginInlineStart: ctx.hingeGapWidth } : undefined}
      >
        {hasSelection ? (
          <scroll key={ctx.selected ?? ''} className={r.scroll.base}>
            <view className={cx(r.detailInner.base, r.detailInner.regular, r.detailInner.width[ctx.contentWidth])}>{children}</view>
          </scroll>
        ) : (
          <view className={r.placeholder.base}>{placeholder ?? null}</view>
        )}
        {actionRow}
      </view>
    </SizeClassScope>
  );
}

/**
 * A list and the detail of the row chosen in it. On a wide screen the two sit side by side, the list
 * at the start (at a fold, on its leaf); on a phone the list fills the screen and choosing a row opens
 * its detail over it, with a way back that Back also takes. Each pane scrolls on its own. It fills its
 * page; put it straight in a `<page>`, without a scroll around it.
 */
export function ListDetail(props: ListDetailProps) {
  const { listLabel, listWidth = 360, contentWidth = 'wide', children, className, 'data-testid': testId, ...rest } = props;
  const { env } = useView();
  const isControlled = typeof props.onChange === 'function';
  const raw = props.bind ? props.bind.value : props.value;
  const selected = raw === undefined || raw === '' ? null : raw;
  const wentBack = useLocal('went-back', false);
  // Back leaves the detail only when something can clear the choice; a `value` given without `onChange` would trap it.
  const canGoBack = props.bind !== undefined || isControlled;
  const clearSelection = () => {
    wentBack.set(true);
    if (props.bind) props.bind.set(null);
    else if (isControlled) props.onChange(null);
  };
  const isCompact = env.sizeClass !== 'regular';
  useBackHandler(isCompact && selected !== null && canGoBack, clearSelection);

  const { hinge, width: pageWidth } = contentHinge(env);
  const verticalHinge = !isCompact && hinge?.orientation === 'vertical' ? hinge : null;
  const hingeGapWidth = verticalHinge ? Math.max(0, verticalHinge.width) : 0;
  // The list leads: at a fold it takes the start leaf, the right one in right-to-left.
  const resolvedListWidth = verticalHinge
    ? Math.max(200, env.direction === 'rtl' ? pageWidth - verticalHinge.x - hingeGapWidth : verticalHinge.x)
    : listWidth;

  return (
    <ListDetailContext.Provider
      value={{ isCompact, selected, clearSelection, canGoBack, wentBack: wentBack.value, listLabel, listWidth: resolvedListWidth, hingeGapWidth, contentWidth, testId }}
    >
      <view data-testid={testId} {...dataOf(rest)} className={cx(r.root.base, isCompact ? r.root.compact : r.root.regular, className)}>
        {children}
      </view>
    </ListDetailContext.Provider>
  );
}

ListDetail.List = ListDetailList;
ListDetail.Detail = ListDetailDetail;
