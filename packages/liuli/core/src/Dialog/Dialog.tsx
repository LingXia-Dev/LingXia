import { cx, useId, type Child } from '@lingxia/view';
import { liuliText } from '../i18n/text';
import { Glyph } from '../internal/Glyph';
import { warnOnce } from '../internal/warn';
import { LayerContext } from '../internal/layer';
import { dialogRecipe as r } from './Dialog.recipe';

export type DialogSize = 'sm' | 'md' | 'lg' | 'full';
export type DialogDismissOn = 'any' | 'closeRequest' | 'none';

export interface DialogProps {
  /** Whether it is shown. A closed dialog renders nothing, so local state inside starts over. */
  isOpen: boolean;
  /** Called with `false` on a close request: Escape, back, the backdrop per `dismissOn`, the close button. */
  onOpenChange: (isOpen: boolean) => void;
  /** The title, and the dialog's accessible name. */
  label: string;
  /** Keeps `label` as the name without drawing it as a title. @default false */
  isLabelHidden?: boolean;
  /** One or two sentences under the title, read as the dialog's description. */
  description?: string;
  /**
   * Interrupts with something to confirm or acknowledge; screen readers announce it as an alert
   * dialog. Pair it with `dismissOn="closeRequest"` or `"none"`. @default false
   */
  isAlert?: boolean;
  /**
   * A picture or an illustration above the title, edge to edge under the panel's top corners: for a
   * dialog that introduces something. Give it its own height or aspect ratio. The close button, if
   * any, sits over it.
   */
  media?: Child;
  /** The body; it scrolls when the panel reaches its maximum height. */
  children?: Child;
  /** The footer, typically Buttons, the main action last. */
  actions?: Child;
  /** Shows a close button in the header. An alert has none unless you ask for one; any other dialog has one unless you turn it off. */
  hasCloseButton?: boolean;
  /** Names the close button. Defaults to "Close" in the page's language. */
  closeLabel?: string;
  /**
   * Which requests close it: `any` adds a backdrop tap to `closeRequest` (Escape, back); `none`
   * leaves closing to your code, for a step that must finish.
   * @default 'any'
   */
  dismissOn?: DialogDismissOn;
  /** Maximum width: 360, 480 or 640. `full` covers the screen, for a task with its own scrolling content: an editor, a long form on a phone. @default 'md' */
  size?: DialogSize;
  /** Classes for the panel. */
  className?: string;
  'data-testid'?: string;
}

const closedBy = { any: 'any', closeRequest: 'closerequest', none: 'none' } as const satisfies Record<DialogDismissOn, string>;

/**
 * A modal panel over the page. Focus moves in when it is opened from a keyboard and returns on
 * close; assistive tech reads only the dialog while it is open.
 */
export function Dialog({
  isOpen,
  onOpenChange,
  label,
  isLabelHidden = false,
  description,
  isAlert = false,
  media,
  children,
  actions,
  hasCloseButton = !isAlert,
  closeLabel = liuliText('@liuli.close'),
  dismissOn = 'any',
  size = 'md',
  className,
  'data-testid': testId,
}: DialogProps) {
  const descriptionId = `${useId()}-description`;
  const close = () => onOpenChange(false);
  if (!hasCloseButton && dismissOn !== 'any' && actions === undefined) {
    warnOnce(`Dialog "${label}" has no way out: no close button, no tap outside, no actions. Set hasCloseButton, or dismissOn="any", or give it an action that closes it.`);
  }

  return (
    <dialog
      open={isOpen}
      onClose={close}
      closedby={closedBy[dismissOn]}
      role={isAlert ? 'alertdialog' : undefined}
      aria-label={label}
      aria-describedby={description !== undefined ? descriptionId : undefined}
      data-testid={testId}
      className={cx(r.panel.base, size !== 'full' && r.panel.floating, r.panel.size[size], className)}
    >
      {media !== undefined && (
        <view className={cx(r.media.base, size !== 'full' && r.media.floating)}>
          <view aria-hidden className={r.media.picture}>{media}</view>
          {hasCloseButton && (
            <button aria-label={closeLabel} onClick={close} className={r.mediaClose.base}>
              <Glyph name="close" className={r.closeIcon.base} />
            </button>
          )}
        </view>
      )}
      {(!isLabelHidden || (hasCloseButton && media === undefined)) && (
        <view className={r.header.base}>
          {!isLabelHidden && <text role="heading" headingLevel={2} className={r.title.base}>{label}</text>}
          {hasCloseButton && media === undefined && (
            <button aria-label={closeLabel} onClick={close} className={r.close.base}>
              <Glyph name="close" className={r.closeIcon.base} />
            </button>
          )}
        </view>
      )}
      {description !== undefined && <text id={descriptionId} className={r.description.base}>{description}</text>}
      {children !== undefined && <LayerContext.Provider value="overlay"><scroll className={cx(r.body.base, size === 'full' && r.body.full)}>{children}</scroll></LayerContext.Provider>}
      {actions !== undefined && <LayerContext.Provider value="overlay"><view className={r.footer.base}>{actions}</view></LayerContext.Provider>}
    </dialog>
  );
}
