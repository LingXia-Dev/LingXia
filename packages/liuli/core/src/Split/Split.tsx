import { SizeClassScope, cx, useView, type Child, type Local } from '@lingxia/view';
import { BottomSheet } from '../BottomSheet/BottomSheet';
import { contentHinge } from '../internal/hinge';
import { splitRecipe as r } from './Split.recipe';

export type SplitCompactMode = 'stack' | 'sheet';

export interface SplitProps {
  /** Primary content (video, document, canvas). */
  children?: Child;
  /** Companion aside content (live chat, comments, inspector). */
  aside: Child;
  /** Aside pane width in `regular` flat mode when no vertical hinge is present. @default 320 */
  asideWidth?: number;
  /** Accessible title for the aside sheet when `compact="sheet"`. @default 'Details' */
  asideTitle?: string;
  /** How `aside` presents on `compact` screens: vertical stack or draggable BottomSheet. @default 'stack' */
  compact?: SplitCompactMode;
  /** Controlled open state for `compact="sheet"`. @default false */
  isOpen?: boolean;
  /** Called when the compact sheet opens or closes. */
  onOpenChange?: (isOpen: boolean) => void;
  /** Uncontrolled local open state cell for `compact="sheet"`. */
  bindOpen?: Local<boolean>;
  className?: string;
  'data-testid'?: string;
}

/**
 * Adaptive primary + companion split layout:
 * - Half-opened horizontal fold (`hinge.orientation === 'horizontal' && hinge.posture === 'half-opened'`):
 *   primary on the upper leaf, companion aside on the lower leaf. The Split must reach the
 *   page bottom (it grows by default) for the seam to meet the crease.
 * - `regular` flat: primary at inline-start, companion aside at inline-end (snaps to vertical hinge when present);
 *   each pane enters `<SizeClassScope>` matching its resolved width (`>= 600` -> `'regular'`, else `'compact'`).
 * - `compact`: stacks vertically (`compact="stack"`) or hosts aside in a `<BottomSheet>` (`compact="sheet"`).
 */
export function Split({
  children,
  aside,
  asideWidth = 320,
  asideTitle = 'Details',
  compact = 'stack',
  isOpen = false,
  onOpenChange,
  bindOpen,
  className,
  'data-testid': testId,
}: SplitProps) {
  const { env } = useView();
  const { hinge, width: viewportWidth, height: contentHeight } = contentHinge(env);

  // Tabletop L-stand posture: primary on upper leaf, aside on lower leaf. The lower leaf is
  // sized from the page bottom, which a Split reaches, so whatever sits above it (a header,
  // page actions) cannot push the seam off the crease.
  if (hinge && hinge.orientation === 'horizontal' && hinge.posture === 'half-opened') {
    const gapHeight = Math.max(0, hinge.height);
    const lowerHeight = Math.max(120, contentHeight - hinge.y - gapHeight);
    const leafScope: 'compact' | 'regular' = viewportWidth >= 600 ? 'regular' : 'compact';
    return (
      <view data-testid={testId} className={cx(r.root.base, r.root.column, className)}>
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
      const sheetOpen = bindOpen !== undefined ? bindOpen.value : isOpen;
      const handleOpenChange = (next: boolean) => {
        if (bindOpen !== undefined) bindOpen.set(next);
        onOpenChange?.(next);
      };
      return (
        <view data-testid={testId} className={cx(r.root.base, r.root.column, className)}>
          <view data-testid={testId && `${testId}-primary`} className={r.primary.base}>
            {children}
          </view>
          <BottomSheet
            isOpen={sheetOpen}
            onOpenChange={handleOpenChange}
            label={asideTitle}
            detents="expandable"
            data-testid={testId && `${testId}-sheet`}
          >
            {aside}
          </BottomSheet>
        </view>
      );
    }

    return (
      <view data-testid={testId} className={cx(r.root.base, r.root.column, className)}>
        <view data-testid={testId && `${testId}-primary`} className={r.primary.base}>
          {children}
        </view>
        <view
          data-testid={testId && `${testId}-aside`}
          className={cx(r.aside.base, r.aside.stacked)}
        >
          {aside}
        </view>
      </view>
    );
  }

  const verticalHinge = hinge && hinge.orientation === 'vertical' ? hinge : null;
  const hingeGapWidth = verticalHinge ? Math.max(0, verticalHinge.width) : 0;
  const resolvedAsideWidth = verticalHinge && viewportWidth > 0
    ? env.direction === 'rtl'
      ? Math.max(240, verticalHinge.x)
      : Math.max(240, viewportWidth - verticalHinge.x - hingeGapWidth)
    : asideWidth;
  const primaryWidth = viewportWidth > 0 ? viewportWidth - resolvedAsideWidth - hingeGapWidth : 0;
  const primaryScope: 'compact' | 'regular' = primaryWidth >= 600 ? 'regular' : 'compact';
  const asideScope: 'compact' | 'regular' = resolvedAsideWidth >= 600 ? 'regular' : 'compact';

  return (
    <view data-testid={testId} className={cx(r.root.base, r.root.row, className)}>
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
          data-testid={testId && `${testId}-aside`}
          className={cx(r.aside.base, r.aside.side)}
          style={{ width: resolvedAsideWidth }}
        >
          {aside}
        </view>
      </SizeClassScope>
    </view>
  );
}
