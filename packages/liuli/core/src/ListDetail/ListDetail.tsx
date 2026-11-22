import {
  SizeClassScope,
  createContext,
  cx,
  useBackHandler,
  useContext,
  useView,
  type Child,
  type Local,
} from '@lingxia/view';
import { Button } from '../Button/Button';
import { contentHinge } from '../internal/hinge';
import { listDetailRecipe as r } from './ListDetail.recipe';

interface ListDetailState {
  isCompact: boolean;
  hasSelection: boolean;
  clearSelection: () => void;
  listLabel: string;
  listWidth: number;
  hingeGapWidth: number;
  testId?: string;
}

const ListDetailContext = createContext<ListDetailState | null>(null);

export interface ListDetailListProps {
  /** The master list content, typically a `<scroll>` or `<List>` of rows. */
  children?: Child;
  className?: string;
}

export interface ListDetailDetailProps {
  /** Detail header title shown in compact drill-down mode and used as the detail group name. */
  title?: string;
  /** Shown on regular screens when no item is selected. */
  placeholder?: Child;
  /** The selected item's detail content. */
  children?: Child;
  className?: string;
}

export interface ListDetailProps {
  /** Controlled selected item id (`null`/`undefined` means no selection). */
  value?: string | null;
  /** Called when selection changes (including Back in compact mode). */
  onChange?: (value: string | null) => void;
  /** Uncontrolled local selection cell (alternative to `value` + `onChange`). */
  bind?: Local<string | null>;
  /** Accessible label for the list pane (also used as the compact back button label). */
  listLabel: string;
  /** Default list pane width in `regular` when no vertical hinge is present. @default 360 */
  listWidth?: number;
  /** `<ListDetail.List>` and `<ListDetail.Detail>` children. */
  children?: Child;
  className?: string;
  'data-testid'?: string;
}

/** The master list slot of a `<ListDetail>`. */
export function ListDetailList({ children, className }: ListDetailListProps) {
  const ctx = useContext(ListDetailContext);
  if (!ctx) throw Error('ListDetail.List must be rendered inside <ListDetail>');

  if (ctx.isCompact) {
    return (
      <view
        role="group"
        aria-label={ctx.listLabel}
        data-testid={ctx.testId && `${ctx.testId}-list`}
        className={cx(
          r.listPane.base,
          r.listPane.compact,
          ctx.hasSelection && r.listPane.hidden,
          className,
        )}
      >
        {children}
      </view>
    );
  }

  return (
    <SizeClassScope value="compact">
      <view
        role="group"
        aria-label={ctx.listLabel}
        data-testid={ctx.testId && `${ctx.testId}-list`}
        className={cx(r.listPane.base, r.listPane.regular, className)}
        style={{ width: ctx.listWidth }}
      >
        {children}
      </view>
    </SizeClassScope>
  );
}

/** The detail pane slot of a `<ListDetail>`. */
export function ListDetailDetail({ title, placeholder, children, className }: ListDetailDetailProps) {
  const ctx = useContext(ListDetailContext);
  if (!ctx) throw Error('ListDetail.Detail must be rendered inside <ListDetail>');

  if (ctx.isCompact) {
    if (!ctx.hasSelection) return null;
    return (
      <view
        role="group"
        aria-label={title ?? ctx.listLabel}
        data-testid={ctx.testId && `${ctx.testId}-detail`}
        className={cx(r.detailPane.base, className)}
      >
        <view className={r.compactHeader.base}>
          <Button
            label={ctx.listLabel}
            startIcon="chevron-start"
            variant="ghost"
            size="sm"
            width="content"
            onClick={ctx.clearSelection}
            data-testid={ctx.testId && `${ctx.testId}-back`}
          />
          {title !== undefined && <text className={r.compactTitle.base}>{title}</text>}
        </view>
        {children}
      </view>
    );
  }

  return (
    <SizeClassScope value="compact">
      <view
        role="group"
        aria-label={title ?? ctx.listLabel}
        data-testid={ctx.testId && `${ctx.testId}-detail`}
        className={cx(r.detailPane.base, className)}
        style={ctx.hingeGapWidth > 0 ? { marginInlineStart: ctx.hingeGapWidth } : undefined}
      >
        {ctx.hasSelection ? children : (placeholder ?? null)}
      </view>
    </SizeClassScope>
  );
}

/**
 * Adaptive master-detail layout:
 * - `regular`: side-by-side list + detail, snapping to a vertical fold hinge when present;
 *   both panes enter `<SizeClassScope value="compact">`.
 * - `compact`: single-pane drill-down with automatic Back handling (`useBackHandler`) and
 *   preserved list scroll state.
 */
export function ListDetail({
  value,
  onChange,
  bind,
  listLabel,
  listWidth = 360,
  children,
  className,
  'data-testid': testId,
}: ListDetailProps) {
  const { env } = useView();
  const selected = bind !== undefined ? bind.value : (value ?? null);
  const hasSelection = selected !== null && selected !== undefined && selected !== '';
  const clearSelection = () => {
    if (bind !== undefined) bind.set(null);
    onChange?.(null);
  };

  const isCompact = env.sizeClass !== 'regular';
  useBackHandler(isCompact && hasSelection, clearSelection);

  const { hinge, width: contentWidth } = contentHinge(env);
  const verticalHinge = !isCompact && hinge?.orientation === 'vertical' ? hinge : null;
  const hingeGapWidth = verticalHinge ? Math.max(0, verticalHinge.width) : 0;
  // The list leads; in RTL it is the leaf right of the crease.
  const resolvedListWidth = verticalHinge
    ? Math.max(
        200,
        env.direction === 'rtl' ? contentWidth - verticalHinge.x - hingeGapWidth : verticalHinge.x,
      )
    : listWidth;

  return (
    <ListDetailContext.Provider
      value={{
        isCompact,
        hasSelection,
        clearSelection,
        listLabel,
        listWidth: resolvedListWidth,
        hingeGapWidth,
        testId,
      }}
    >
      <view
        data-testid={testId}
        className={cx(r.root.base, isCompact ? r.root.compact : r.root.regular, className)}
      >
        {children}
      </view>
    </ListDetailContext.Provider>
  );
}

ListDetail.List = ListDetailList;
ListDetail.Detail = ListDetailDetail;
