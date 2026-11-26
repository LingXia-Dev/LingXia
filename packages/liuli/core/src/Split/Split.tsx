import { SizeClassScope, cx, useBox, useLocal, useView, type Child, type Local, type PointerEvent } from '@lingxia/view';
import { liuliText } from '../i18n/text';
import { BottomSheet } from '../BottomSheet/BottomSheet';
import { dataOf } from '../internal/data';
import { contentHinge } from '../internal/hinge';
import { splitRecipe as r } from './Split.recipe';

export type SplitCompactMode = 'stack' | 'sheet';

// The main content's least width beside a dragged aside, and an arrow key's step, in points.
const leastPrimary = 320;
const keyStep = 16;

/** How the aside shows on a phone: under the main content, or in a sheet the page opens. */
export type SplitCompact =
  | {
      /** `stack` puts the aside under the main content; `sheet` puts it in a bottom sheet the page opens. @default 'stack' */
      compact?: 'stack';
      bind?: never;
      isOpen?: never;
      onOpenChange?: never;
    }
  | {
      compact: 'sheet';
      /** A View-local cell: whether the sheet is open. */
      bind: Local<boolean>;
      isOpen?: never;
      onOpenChange?: never;
    }
  | {
      compact: 'sheet';
      bind?: never;
      /** Whether the sheet is open. */
      isOpen: boolean;
      /** Runs with `false` when the sheet is dismissed. */
      onOpenChange: (isOpen: boolean) => void;
    };

export type SplitProps = SplitCompact & {
  /** The main content: a video, a document, a canvas. */
  children?: Child;
  /** The companion beside it: a live chat, comments, an inspector. */
  aside: Child;
  /** Names the companion for a screen reader in every form, beside, below or in a sheet: "Live chat", "Comments". Not drawn; start `aside` with its own heading. */
  asideLabel: string;
  /** The aside's width in pt beside the main content, when no fold crosses the page; with `isResizable`, where it starts. @default 320 */
  asideWidth?: number;
  /**
   * The aside's edge can be dragged, or stepped with the arrow keys, to widen or narrow it beside
   * the main content, which keeps at least 320 pt. Not on a phone or at a fold. @default false
   */
  isResizable?: boolean;
  /** The narrowest the aside is dragged to. @default 240 */
  minAsideWidth?: number;
  /** The widest the aside is dragged to. @default 480 */
  maxAsideWidth?: number;
  /** Runs with the aside's width when a drag or a key step ends: to keep it for the next visit. */
  onAsideWidthChange?: (width: number) => void;
  className?: string;
  /** On the root; the panes get `<testid>-primary` and `<testid>-aside`, the sheet `<testid>-sheet`. */
  'data-testid'?: string;
};

/**
 * The main content and a companion beside it. On a wide screen the aside sits at the end (at a fold, on
 * the end leaf); on a foldable stood half open, the main content takes the upper half and the aside the
 * lower, so the Split must reach the page's bottom for the seam to meet the crease; on a phone the aside
 * stacks under the main content or opens in a sheet. Each pane's contents size themselves to the pane.
 */
export function Split(props: SplitProps) {
  const {
    children, aside, asideLabel, asideWidth = 320, isResizable = false, minAsideWidth = 240, maxAsideWidth = 480, onAsideWidthChange,
    compact = 'stack', className, 'data-testid': testId, ...rest
  } = props;
  const attributes = dataOf(rest);
  const { env } = useView();
  const { hinge, width: viewportWidth, height: contentHeight } = contentHinge(env);
  const dragged = useLocal<number | null>('asideWidth', null);
  const grab = useLocal<{ x: number; width: number } | null>('grab', null);
  const isOver = useLocal('handleOver', false);
  const root = useBox('split');

  // Tabletop L-stand posture: primary on upper leaf, aside on lower leaf. The lower leaf is
  // sized from the page bottom, which a Split reaches, so whatever sits above it (a header,
  // page actions) cannot push the seam off the crease.
  if (hinge && hinge.orientation === 'horizontal' && hinge.posture === 'half-opened') {
    const gapHeight = Math.max(0, hinge.height);
    const lowerHeight = Math.max(120, contentHeight - hinge.y - gapHeight);
    const leafScope: 'compact' | 'regular' = viewportWidth >= 600 ? 'regular' : 'compact';
    return (
      <view data-testid={testId} {...attributes} className={cx(r.root.base, r.root.column, className)}>
        <SizeClassScope value={leafScope}>
          <view data-testid={testId && `${testId}-primary`} className={r.primary.tabletop}>
            {children}
          </view>
        </SizeClassScope>
        {gapHeight > 0 && (
          <view aria-hidden className={r.hingeGap.base} style={{ height: gapHeight }} />
        )}
        <SizeClassScope value={leafScope}>
          <view
            role="group"
            aria-label={asideLabel}
            data-testid={testId && `${testId}-aside`}
            className={cx(r.aside.base, r.aside.tabletop)}
            style={{ height: lowerHeight }}
          >
            {aside}
          </view>
        </SizeClassScope>
      </view>
    );
  }

  if (env.sizeClass !== 'regular') {
    if (compact === 'sheet') {
      const sheetOpen = props.bind ? props.bind.value : props.isOpen === true;
      const handleOpenChange = (next: boolean) => {
        if (props.bind) props.bind.set(next);
        else props.onOpenChange?.(next);
      };
      return (
        <view data-testid={testId} {...attributes} className={cx(r.root.base, r.root.column, className)}>
          <view data-testid={testId && `${testId}-primary`} className={r.primary.base}>
            {children}
          </view>
          <BottomSheet
            isOpen={sheetOpen}
            onOpenChange={handleOpenChange}
            label={asideLabel}
            isLabelHidden
            detents="expandable"
            initialDetent="full"
            data-testid={testId && `${testId}-sheet`}
          >
            {aside}
          </BottomSheet>
        </view>
      );
    }

    return (
      <view data-testid={testId} {...attributes} className={cx(r.root.base, r.root.column, className)}>
        <view data-testid={testId && `${testId}-primary`} className={r.primary.base}>
          {children}
        </view>
        <view
          role="group"
          aria-label={asideLabel}
          data-testid={testId && `${testId}-aside`}
          className={cx(r.aside.base, r.aside.stacked)}
        >
          {aside}
        </view>
      </view>
    );
  }

  // A dragged aside keeps between its bounds and leaves the main content its least width.
  const widest = () => Math.max(minAsideWidth, Math.min(maxAsideWidth, (root.value?.width ?? Infinity) - leastPrimary));
  const fitted = (width: number) => Math.round(Math.min(widest(), Math.max(minAsideWidth, width)));
  const verticalHinge = hinge && hinge.orientation === 'vertical' ? hinge : null;
  const hingeGapWidth = verticalHinge ? Math.max(0, verticalHinge.width) : 0;
  const resolvedAsideWidth = verticalHinge && viewportWidth > 0
    ? env.direction === 'rtl'
      ? Math.max(240, verticalHinge.x)
      : Math.max(240, viewportWidth - verticalHinge.x - hingeGapWidth)
    : isResizable ? fitted(dragged.value ?? asideWidth) : asideWidth;
  const primaryWidth = viewportWidth > 0 ? viewportWidth - resolvedAsideWidth - hingeGapWidth : 0;
  const primaryScope: 'compact' | 'regular' = primaryWidth >= 600 ? 'regular' : 'compact';
  const asideScope: 'compact' | 'regular' = resolvedAsideWidth >= 600 ? 'regular' : 'compact';

  const isDragging = !verticalHinge && isResizable;
  const handle = isDragging && (() => {
    // The aside is at the end; in a right-to-left page that end is on the left, so a drag right narrows it there.
    const outward = env.direction === 'rtl' ? 1 : -1;
    const onPointer = (event: PointerEvent) => {
      const { phase } = event;
      if (phase === 'hover') return isOver.value ? undefined : isOver.set(true);
      if (phase === 'leave') return isOver.set(false);
      if (phase === 'increment' || phase === 'decrement') {
        const next = fitted(resolvedAsideWidth + (phase === 'increment' ? keyStep : -keyStep));
        dragged.set(next);
        onAsideWidthChange?.(next);
        return;
      }
      if (phase === 'down') return grab.set({ x: event.x, width: resolvedAsideWidth });
      const from = grab.value;
      if (!from) return;
      if (phase === 'move') return dragged.set(fitted(from.width + outward * (event.x - from.x)));
      grab.set(null);
      // A cancelled drag puts the aside back; a release keeps it where the pointer let go.
      if (phase === 'cancel') return dragged.set(from.width);
      if (phase === 'up') {
        const next = fitted(from.width + outward * (event.x - from.x));
        dragged.set(next);
        onAsideWidthChange?.(next);
      }
    };
    return (
      <view
        onPointer={onPointer}
        pointerAxis="x"
        aria-label={liuliText('@liuli.resizePane', { name: asideLabel })}
        aria-valuenow={Math.round(resolvedAsideWidth)}
        aria-valuemin={minAsideWidth}
        aria-valuemax={widest()}
        data-testid={testId && `${testId}-handle`}
        className={r.handle.base}
      >
        <view className={cx(r.handleLine.base, r.handleLine.state[grab.value ? 'held' : isOver.value ? 'hover' : 'rest'])} />
      </view>
    );
  })();

  return (
    <view box={root} data-testid={testId} {...attributes} className={cx(r.root.base, r.root.row, className)}>
      <SizeClassScope value={primaryScope}>
        <view data-testid={testId && `${testId}-primary`} className={r.primary.base}>
          {children}
        </view>
      </SizeClassScope>
      {hingeGapWidth > 0 && (
        <view aria-hidden className={r.hingeGap.base} style={{ width: hingeGapWidth }} />
      )}
      <SizeClassScope value={asideScope}>
        <view
          role="group"
          aria-label={asideLabel}
          data-testid={testId && `${testId}-aside`}
          className={cx(r.aside.base, r.aside.side)}
          style={{ width: resolvedAsideWidth }}
        >
          {aside}
          {/* Last, so it lies over the aside's own content and reaches back over the main content's edge. */}
          {handle}
        </view>
      </SizeClassScope>
    </view>
  );
}
