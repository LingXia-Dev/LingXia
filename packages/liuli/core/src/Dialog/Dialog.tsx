import { cx, useId, useLocal, type Child } from '@lingxia/view';
import { Button } from '../Button/Button';
import { reportFailure, settleFlag } from '../internal/settle';
import { liuliText } from '../i18n/text';
import { Glyph } from '../internal/Glyph';
import { warnOnce } from '../internal/warn';
import { LayerContext } from '../internal/layer';
import { dialogRecipe as r } from './Dialog.recipe';
import { dataOf } from '../internal/data';
import { Footer, useBodyEdges } from '../internal/footer';

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
   * dialog. A backdrop tap does not close it; Escape and back still do, as its cancel. @default false
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
  /** Beside the title, before the close button: one small control that acts on the whole panel, such as Refresh. */
  headerAction?: Child;
  /** Shows a close button in the header. An alert has none unless you ask for one; any other dialog has one unless you turn it off. */
  hasCloseButton?: boolean;
  /** Names the close button. Defaults to "Close" in the page's language. */
  closeLabel?: string;
  /**
   * Which requests close it: `any` adds a backdrop tap to `closeRequest` (Escape, back); `none`
   * leaves closing to your code, for a step that must finish. An alert takes `closeRequest` unless you
   * say otherwise; any other dialog takes `any`.
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
  headerAction,
  hasCloseButton = !isAlert,
  closeLabel = liuliText('@liuli.close'),
  dismissOn: ownDismissOn,
  size = 'md',
  className,
  'data-testid': testId, ...attributes
}: DialogProps) {
  const bodyEdges = useBodyEdges();
  const descriptionId = `${useId()}-description`;
  const close = () => onOpenChange(false);
  const dismissOn = ownDismissOn ?? (isAlert ? 'closeRequest' : 'any');
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
      data-testid={testId} {...dataOf(attributes)}
      className={cx(r.panel.base, size !== 'full' && r.panel.floating, r.panel.size[size], className)}
    >
      {media !== undefined && (
        <view className={cx(r.media.base, size !== 'full' && r.media.floating)}>
          <view aria-hidden className={r.media.picture}>{media}</view>
          {hasCloseButton && (
            <button aria-label={closeLabel} onClick={close} data-testid={testId && `${testId}-close`} className={r.mediaClose.base}>
              <Glyph name="close" className={r.closeIcon.base} />
            </button>
          )}
        </view>
      )}
      {(!isLabelHidden || headerAction !== undefined || (hasCloseButton && media === undefined)) && (
        <view className={cx(r.header.base, isLabelHidden && r.header.bare)}>
          {!isLabelHidden && <text role="heading" headingLevel={2} className={r.title.base}>{label}</text>}
          {headerAction !== undefined && <view className={r.headerAction.base}>{headerAction}</view>}
          {hasCloseButton && media === undefined && (
            <button aria-label={closeLabel} onClick={close} data-testid={testId && `${testId}-close`} className={r.close.base}>
              <Glyph name="close" className={r.closeIcon.base} />
            </button>
          )}
        </view>
      )}
      {description !== undefined && <text id={descriptionId} className={r.description.base}>{description}</text>}
      {children !== undefined && <LayerContext.Provider value="overlay"><scroll className={cx(r.body.base, size === 'full' && r.body.full, bodyEdges.className)} onScrollEdge={bodyEdges.onScrollEdge}>{children}</scroll></LayerContext.Provider>}
      {actions !== undefined && <Footer className={r.footer.base}>{actions}</Footer>}
    </dialog>
  );
}

export interface AlertDialogProps {
  /** Whether it is shown. */
  isOpen: boolean;
  /** Called with `false` on cancel, Escape or back, and after `confirmAction` succeeds. */
  onOpenChange: (isOpen: boolean) => void;
  /** The question, as the title and the dialog's accessible name ("Delete this note?"). */
  label: string;
  /** What happens if the person goes on. */
  description?: string;
  /** Names the action by its outcome ("Delete"). */
  confirmLabel: string;
  /** Runs the action. The dialog stays open with the button loading until it settles; it closes on success and shows `failedMessage` on failure. */
  confirmAction: () => Promise<unknown>;
  /** Draws the action in the danger colour, for something that cannot be undone. @default false */
  isDestructive?: boolean;
  /** Names the cancel button. Defaults to "Cancel" in the page's language. */
  cancelLabel?: string;
  /** Shown in the dialog when `confirmAction` fails; the person can try again or cancel. Defaults to "That didn't work. Try again." in the page's language. */
  failedMessage?: string;
  'data-testid'?: string;
}

/**
 * A confirmation with two answers. Cancel comes first and takes focus from a keyboard; Escape and
 * back cancel, a backdrop tap does nothing. While the action runs, nothing closes it.
 */
export function AlertDialog({
  isOpen,
  onOpenChange,
  label,
  description,
  confirmLabel,
  confirmAction,
  isDestructive = false,
  cancelLabel = liuliText('@liuli.cancel'),
  failedMessage = liuliText('@liuli.actionFailed'),
  'data-testid': testId,
  ...attributes
}: AlertDialogProps) {
  const pending = useLocal('pending', false);
  const failed = useLocal('failed', false);
  const change = (next: boolean) => {
    if (pending.value) return;
    if (!next) failed.set(false);
    onOpenChange(next);
  };
  const confirm = async () => {
    if (pending.value) return;
    pending.set(true);
    failed.set(false);
    try {
      await confirmAction();
      settleFlag(pending, false);
      onOpenChange(false);
    } catch (error) {
      reportFailure('AlertDialog', label, error);
      settleFlag(failed, true);
      settleFlag(pending, false);
    }
  };
  return (
    <Dialog
      isOpen={isOpen}
      onOpenChange={change}
      label={label}
      description={description}
      isAlert
      dismissOn={pending.value ? 'none' : 'closeRequest'}
      size="sm"
      data-testid={testId} {...dataOf(attributes)}
      actions={
        <>
          <Button label={cancelLabel} isDisabled={pending.value} onClick={() => change(false)} data-testid={testId && `${testId}-cancel`} />
          <Button label={confirmLabel} variant={isDestructive ? 'destructive' : 'primary'} isLoading={pending.value} onClick={confirm} data-testid={testId && `${testId}-confirm`} />
        </>
      }
      children={failed.value ? (
        <view role="alert" className={r.failed.base} data-testid={testId && `${testId}-failed`}>
          <Glyph name="error" className={r.failedIcon.base} />
          <text className={r.failedText.base}>{failedMessage}</text>
        </view>
      ) : undefined}
    />
  );
}
