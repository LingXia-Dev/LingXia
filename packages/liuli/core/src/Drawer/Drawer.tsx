import { cx, useId, type Child } from '@lingxia/view';
import type { DialogDismissOn } from '../Dialog/Dialog';
import { liuliText } from '../i18n/text';
import { Glyph } from '../internal/Glyph';
import { warnOnce } from '../internal/warn';
import { LayerContext } from '../internal/layer';
import { drawerRecipe as r } from './Drawer.recipe';
import { dataOf } from '../internal/data';
import { Footer, useBodyEdges } from '../internal/footer';

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
  /** Beside the title, before the close button: one small control that acts on the whole panel, such as Refresh. */
  headerAction?: Child;
  /** Shows a close button. @default true */
  hasCloseButton?: boolean;
  /** Names the close button. Defaults to "Close" in the page's language. */
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
  headerAction,
  hasCloseButton = true,
  closeLabel = liuliText('@liuli.close'),
  side = 'end',
  size = 'md',
  dismissOn = 'any',
  className,
  'data-testid': testId, ...attributes
}: DrawerProps) {
  if (!hasCloseButton && dismissOn !== 'any' && actions === undefined) {
    warnOnce(`Drawer "${label}" has no way out: no close button, no tap outside, no actions. Set hasCloseButton, or dismissOn="any", or give it an action that closes it.`);
  }
  const bodyEdges = useBodyEdges();
  const descriptionId = `${useId()}-description`;
  const close = () => onOpenChange(false);
  return (
    <dialog
      open={isOpen}
      onClose={close}
      closedby={closedBy[dismissOn]}
      aria-label={label}
      aria-describedby={description !== undefined ? descriptionId : undefined}
      data-testid={testId} {...dataOf(attributes)}
      className={cx(r.panel.base, r.panel.side[side], r.panel.size[size], className)}
    >
      <view className={r.header.base}>
        <text role="heading" headingLevel={2} className={r.title.base}>{label}</text>
        {headerAction !== undefined && <view className={r.headerAction.base}>{headerAction}</view>}
        {hasCloseButton && (
          <button aria-label={closeLabel} onClick={close} className={r.close.base}>
            <Glyph name="close" className={r.closeIcon.base} />
          </button>
        )}
      </view>
      {description !== undefined && <text id={descriptionId} className={r.description.base}>{description}</text>}
      <LayerContext.Provider value="overlay"><scroll className={cx(r.body.base, bodyEdges.className)} onScrollEdge={bodyEdges.onScrollEdge}>{children}</scroll></LayerContext.Provider>
      {actions !== undefined && <Footer className={r.footer.base}>{actions}</Footer>}
    </dialog>
  );
}
