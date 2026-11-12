import { cx, useLocal, type Child } from '@lingxia/view';
import { glyphBox, glyphs } from '../internal/glyphs';
import { settleFlag } from '../internal/settle';
import { buttonRecipe as r } from './Button.recipe';

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'destructive';
export type ButtonSize = 'sm' | 'md' | 'lg';
export type ButtonWidth = 'content' | 'fill';

export interface ButtonProps {
  /** The visible text and the accessible name. Name the outcome: "Save changes", not "OK". */
  label: string;
  /**
   * `primary` for the one main action in view, `destructive` for an action that loses data.
   * @default 'secondary'
   */
  variant?: ButtonVariant;
  /** @default 'md' */
  size?: ButtonSize;
  /** Drawn before the label; hidden from assistive tech, since the label names the button. */
  startIcon?: Child;
  /** Drawn after the label: a count, a shortcut hint. Hidden from assistive tech. */
  endContent?: Child;
  /** `content` keeps its own width in a column; `fill` stretches across. Unset follows the parent. */
  width?: ButtonWidth;
  /** @default false */
  isDisabled?: boolean;
  /** Why it is disabled. Read after the label while disabled; not drawn. */
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

/** Runs an action when pressed. */
export function Button({
  label,
  variant = 'secondary',
  size = 'md',
  startIcon,
  endContent,
  width,
  isDisabled = false,
  disabledMessage,
  isLoading = false,
  onClick,
  clickAction,
  className,
  'data-testid': testId,
}: ButtonProps) {
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
      aria-label={isDisabled && disabledMessage ? `${label}, ${disabledMessage}` : label}
      aria-busy={busy || undefined}
      disabled={isDisabled}
      onClick={press}
      className={cx(r.root.base, r.root.variant[variant], r.root.size[size], width && r.root.width[width], className)}
    >
      {busy ? (
        <vector aria-hidden paths={glyphs.spinner} viewBox={glyphBox} className={cx(r.spinner.base, r.spinner.size[size])} />
      ) : (
        startIcon !== undefined && <view aria-hidden className={r.icon.base}>{startIcon}</view>
      )}
      <text className={cx(r.label.base, r.label.size[size])}>{label}</text>
      {endContent !== undefined && <view aria-hidden className={r.icon.base}>{endContent}</view>}
    </button>
  );
}
