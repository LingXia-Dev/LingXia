import { cx, useId, useView, type Child } from '@lingxia/view';
import type { DialogDismissOn } from '../Dialog/Dialog';
import { Glyph } from '../internal/Glyph';
import { LayerContext } from '../internal/layer';
import { drawerRecipe as r } from './Drawer.recipe';

export type DrawerSide = 'end' | 'start';
export type DrawerSize = 'sm' | 'md' | 'lg';

export interface DrawerProps {
  /** Whether it is shown. A closed drawer renders nothing, so local state inside starts over. */
  isOpen: boolean;
  /** Called with `false` on a close request: Escape, back, the backdrop per `dismissOn`, the close button. */
  onOpenChange: (isOpen: boolean) => void;
  /** The title, and the drawer's accessible name. */
  label: string;
  /** A sentence under the title, read as the drawer's description. */
  description?: string;
  /** The body; it scrolls when it is taller than the drawer. */
  children?: Child;
  /** The footer, typically Buttons, the main action last. */
  actions?: Child;
  /** Shows a close button in the header, named by this text. */
  closeLabel?: string;
  /** `end` for details and filters of what is on the page; `start` for navigation. @default 'end' */
  side?: DrawerSide;
  /** Maximum width: 320, 420 or 560; never more than most of the screen. @default 'md' */
  size?: DrawerSize;
  /** @default 'any' */
  dismissOn?: DialogDismissOn;
  /** Classes for the panel. */
  className?: string;
  'data-testid'?: string;
}

const closedBy = { any: 'any', closeRequest: 'closerequest', none: 'none' } as const satisfies Record<DialogDismissOn, string>;

/** A modal panel on a side edge, the height of the screen: room for a form or a record beside the page it came from. */
export function Drawer({
  isOpen,
  onOpenChange,
  label,
  description,
  children,
  actions,
  closeLabel,
  side = 'end',
  size = 'md',
  dismissOn = 'any',
  className,
  'data-testid': testId,
}: DrawerProps) {
  const descriptionId = `${useId()}-description`;
  const { env } = useView();
  const close = () => onOpenChange(false);
  return (
    <dialog
      open={isOpen}
      onClose={close}
      closedby={closedBy[dismissOn]}
      aria-label={label}
      aria-describedby={description !== undefined ? descriptionId : undefined}
      data-testid={testId}
      // It runs under the system bars, so its own padding clears them.
      style={{ paddingTop: env.safeArea.top + 20, paddingBottom: env.safeArea.bottom + 20 }}
      className={cx(r.panel.base, r.panel.side[side], r.panel.size[size], className)}
    >
      <view className={r.header.base}>
        <text role="heading" headingLevel={2} className={r.title.base}>{label}</text>
        {closeLabel !== undefined && (
          <button aria-label={closeLabel} onClick={close} className={r.close.base}>
            <Glyph name="close" className={r.closeIcon.base} />
          </button>
        )}
      </view>
      {description !== undefined && <text id={descriptionId} className={r.description.base}>{description}</text>}
      <LayerContext.Provider value="overlay"><scroll className={r.body.base}>{children}</scroll></LayerContext.Provider>
      {actions !== undefined && <view className={r.footer.base}>{actions}</view>}
    </dialog>
  );
}
