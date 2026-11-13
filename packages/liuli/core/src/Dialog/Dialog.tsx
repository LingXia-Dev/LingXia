import { cx, useId, type Child } from '@lingxia/view';
import { glyphBox, glyphs } from '../internal/glyphs';
import { LayerContext } from '../internal/layer';
import { dialogRecipe as r } from './Dialog.recipe';

export type DialogSize = 'sm' | 'md' | 'lg';
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
  /** The body; it scrolls when the panel reaches its maximum height. */
  children?: Child;
  /** The footer, typically Buttons, the main action last. */
  actions?: Child;
  /** Shows a close button in the header, named by this text. */
  closeLabel?: string;
  /**
   * Which requests close it: `any` adds a backdrop tap to `closeRequest` (Escape, back); `none`
   * leaves closing to your code, for a step that must finish.
   * @default 'any'
   */
  dismissOn?: DialogDismissOn;
  /** Maximum width: 360, 480 or 640. @default 'md' */
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
  children,
  actions,
  closeLabel,
  dismissOn = 'any',
  size = 'md',
  className,
  'data-testid': testId,
}: DialogProps) {
  const descriptionId = `${useId()}-description`;
  const close = () => onOpenChange(false);
  return (
    <dialog
      open={isOpen}
      onClose={close}
      closedby={closedBy[dismissOn]}
      role={isAlert ? 'alertdialog' : undefined}
      aria-label={label}
      aria-describedby={description !== undefined ? descriptionId : undefined}
      data-testid={testId}
      className={cx(r.panel.base, r.panel.size[size], className)}
    >
      {(!isLabelHidden || closeLabel !== undefined) && (
        <view className={r.header.base}>
          {!isLabelHidden && <text role="heading" headingLevel={2} className={r.title.base}>{label}</text>}
          {closeLabel !== undefined && (
            <button aria-label={closeLabel} onClick={close} className={r.close.base}>
              <vector aria-hidden paths={glyphs.close} viewBox={glyphBox} className={r.closeIcon.base} />
            </button>
          )}
        </view>
      )}
      {description !== undefined && <text id={descriptionId} className={r.description.base}>{description}</text>}
      {children !== undefined && <LayerContext.Provider value="raised"><scroll className={r.body.base}>{children}</scroll></LayerContext.Provider>}
      {actions !== undefined && <view className={r.footer.base}>{actions}</view>}
    </dialog>
  );
}
