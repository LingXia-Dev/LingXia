import { cx, useId, useLocal, type Child } from '@lingxia/view';
import type { DialogDismissOn } from '../Dialog/Dialog';
import { Glyph } from '../internal/Glyph';
import { LayerContext } from '../internal/layer';
import { bottomSheetRecipe as r } from './BottomSheet.recipe';

/** `fit` is as tall as its content; `expandable` opens halfway and drags up to nearly full height. */
export type BottomSheetDetents = 'fit' | 'expandable';

export interface BottomSheetProps {
  /** Whether it is shown. A closed sheet renders nothing, so local state inside starts over. */
  isOpen: boolean;
  /** Called with `false` on a close request: Escape, back, the backdrop or a drag off screen per `dismissOn`, the close button. */
  onOpenChange: (isOpen: boolean) => void;
  /** The title, and the sheet's accessible name. */
  label: string;
  /** Keeps `label` as the name without drawing it as a title. @default false */
  isLabelHidden?: boolean;
  /** One or two sentences under the title, read as the sheet's description. */
  description?: string;
  /** The body; it scrolls when the sheet reaches its height. */
  children?: Child;
  /** The footer, typically Buttons, the main action last. */
  actions?: Child;
  /** Shows a close button in the header, named by this text. */
  closeLabel?: string;
  /** Which requests close it; with `none` it cannot be dragged off either. @default 'any' */
  dismissOn?: DialogDismissOn;
  /** @default 'fit' */
  detents?: BottomSheetDetents;
  /** Classes for the panel. */
  className?: string;
  'data-testid'?: string;
}

const closedBy = { any: 'any', closeRequest: 'closerequest', none: 'none' } as const satisfies Record<DialogDismissOn, string>;

// Offsets down from where the sheet sits, as a share of its height; the last is off screen.
const points = {
  fit: { stay: [0], leave: [0, '100%'], open: 0 },
  expandable: { stay: [0, '45%'], leave: [0, '45%', '100%'], open: 1 },
} as const;

/**
 * A modal panel on the bottom edge. It follows a drag between its detents, and a drag off screen
 * closes it. Assistive tech reads it as a dialog whose position can be stepped.
 */
export function BottomSheet({
  isOpen,
  onOpenChange,
  label,
  isLabelHidden = false,
  description,
  children,
  actions,
  closeLabel,
  dismissOn = 'any',
  detents = 'fit',
  className,
  'data-testid': testId,
}: BottomSheetProps) {
  const descriptionId = `${useId()}-description`;
  const shape = points[detents];
  const detent = useLocal(`detent-${detents}`, shape.open);
  const close = () => {
    // The next open starts at the opening detent, however this one ended.
    detent.set(shape.open);
    onOpenChange(false);
  };
  return (
    <dialog
      open={isOpen}
      onClose={close}
      closedby={closedBy[dismissOn]}
      aria-label={label}
      aria-describedby={description !== undefined ? descriptionId : undefined}
      drag="y"
      dragSnapPoints={dismissOn === 'none' ? shape.stay : shape.leave}
      bind={detent}
      data-testid={testId}
      className={cx(r.panel.base, r.panel.detents[detents], className)}
    >
      <view aria-hidden className={r.grabber.base} />
      {(!isLabelHidden || closeLabel !== undefined) && (
        <view className={r.header.base}>
          {!isLabelHidden && <text role="heading" headingLevel={2} className={r.title.base}>{label}</text>}
          {closeLabel !== undefined && (
            <button aria-label={closeLabel} onClick={close} data-testid={testId && `${testId}-close`} className={r.close.base}>
              <Glyph name="close" className={r.closeIcon.base} />
            </button>
          )}
        </view>
      )}
      {description !== undefined && <text id={descriptionId} className={r.description.base}>{description}</text>}
      {children !== undefined && <LayerContext.Provider value="overlay"><scroll className={r.body.base}>{children}</scroll></LayerContext.Provider>}
      {actions !== undefined && <LayerContext.Provider value="overlay"><view className={r.footer.base}>{actions}</view></LayerContext.Provider>}
    </dialog>
  );
}
