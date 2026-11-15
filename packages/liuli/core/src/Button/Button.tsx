import { createContext, cx, useContext, useId, useLocal, type Child } from '@lingxia/view';
import type { IconName } from '../Icon/Icon';
import { Glyph } from '../internal/Glyph';
import { spinner, spinnerBox } from '../internal/spinner';
import { settleFlag } from '../internal/settle';
import { buttonRecipe as r } from './Button.recipe';

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'link' | 'destructive';
export type ButtonSize = 'sm' | 'md' | 'lg';
export type ButtonWidth = 'content' | 'fill';

export interface ButtonProps {
  /** The visible text and the accessible name. Name the outcome: "Save changes", not "OK". */
  label: string;
  /**
   * In a ButtonGroup the group's variant, unless set here. `primary` for the one main action in view, `destructive` for an action that loses data, `link`
   * for a quiet way out beside a form's main action ("Forgot password?").
   * @default 'secondary'
   */
  variant?: ButtonVariant;
  /** In a ButtonGroup the group's size, unless set here. @default 'md' */
  size?: ButtonSize;
  /** An icon before the label, by name; hidden from assistive tech, since the label names the button. */
  startIcon?: IconName;
  /** An icon after the label: a chevron for a button that opens something, an arrow for one that goes on. */
  endIcon?: IconName;
  /** Draws `startIcon` alone in a square; `label` stays the accessible name. For a named icon button use IconButton. @default false */
  isIconOnly?: boolean;
  /** Drawn after the label: a count, a shortcut hint. Hidden from assistive tech. */
  endContent?: Child;
  /** `content` keeps its own width in a column; `fill` stretches across. Unset follows the parent. */
  width?: ButtonWidth;
  /** @default false */
  isDisabled?: boolean;
  /** Why it is disabled: read as the button's description while disabled; not drawn. */
  disabledMessage?: string;
  /** Shows a spinner and ignores presses; the button stays focusable and reports busy. @default false */
  isLoading?: boolean;
  /** Runs on press, before `clickAction`. */
  onClick?: () => void;
  /** An async action, such as a Logic action. The button shows loading until it settles. */
  clickAction?: () => Promise<unknown>;
  /** Merged last, so its classes win over the recipe's. */
  className?: string;
  'data-testid'?: string;
}

export interface ButtonGroupState {
  /** The buttons share edges and round only the group's two ends. */
  isAttached: boolean;
  size?: ButtonSize;
  variant?: ButtonVariant;
  isDisabled: boolean;
}

/** Set by ButtonGroup for the buttons inside it. */
export const ButtonGroupContext = createContext<ButtonGroupState | null>(null);

/** Runs an action when pressed. */
export function Button({
  label,
  variant: ownVariant,
  size: ownSize,
  startIcon,
  endIcon,
  isIconOnly = false,
  endContent,
  width,
  isDisabled: ownDisabled = false,
  disabledMessage,
  isLoading = false,
  onClick,
  clickAction,
  className,
  'data-testid': testId,
}: ButtonProps) {
  const group = useContext(ButtonGroupContext);
  const variant = ownVariant ?? group?.variant ?? 'secondary';
  const size = ownSize ?? group?.size ?? 'md';
  const isDisabled = ownDisabled || group?.isDisabled === true;
  const reasonId = `${useId()}-reason`;
  const hasReason = isDisabled && disabledMessage !== undefined;
  const pending = useLocal('pending', false);
  const busy = isLoading || pending.value;
  // Returning the promise lets the View report a rejected action as a handler error.
  const press = async () => {
    if (busy) return;
    onClick?.();
    if (!clickAction) return;
    pending.set(true);
    try {
      await clickAction();
    } finally {
      settleFlag(pending, false);
    }
  };
  return (
    <button
      data-testid={testId}
      aria-label={label}
      aria-describedby={hasReason ? reasonId : undefined}
      aria-busy={busy || undefined}
      disabled={isDisabled}
      onClick={press}
      className={cx(
        r.root.base, r.root.variant[variant], r.root.disabled[variant], isIconOnly ? r.root.square[size] : r.root.size[size],
        group?.isAttached && r.root.attached, width && r.root.width[width], className,
      )}
    >
      {busy ? (
        <vector aria-hidden paths={spinner} viewBox={spinnerBox} className={cx(r.spinner.base, r.spinner.size[size])} />
      ) : (
        startIcon !== undefined && <Glyph name={startIcon} className={r.glyph.size[size]} />
      )}
      {/* An icon-only button keeps its label as the name; nothing else is drawn beside the icon. */}
      {!isIconOnly && <text className={cx(r.label.base, r.label.weight[variant], r.label.size[size])}>{label}</text>}
      {!isIconOnly && endIcon !== undefined && <Glyph name={endIcon} className={r.glyph.size[size]} />}
      {!isIconOnly && endContent !== undefined && <view aria-hidden className={r.icon.base}>{endContent}</view>}
      {hasReason && <text id={reasonId} aria-hidden className={r.reason.base}>{disabledMessage}</text>}
    </button>
  );
}
