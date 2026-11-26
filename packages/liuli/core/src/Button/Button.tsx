import { createContext, cx, useContext, useId, useLocal, type Child, type ColorValue, type LinkTarget } from '@lingxia/view';
import type { Press } from '../internal/press';
import type { IconRef } from '../Icon/Icon';
import { Glyph, glyphGrowth, useTextGrowth } from '../internal/Glyph';
import { ActionEdge } from '../internal/actionEdge';
import { SpinnerMark } from '../internal/spinner';

// On a solid fill the spinner's ring is the fill's own on-colour, faint.
const spinnerRing: Partial<Record<ButtonVariant, ColorValue>> = { primary: 'on-accent-solid/30', destructive: 'on-danger-solid/30' };
import { reportFailure, settleFlag } from '../internal/settle';
import { registerPressable, identityOf, useTrigger } from '../internal/trigger';
import { dataOf } from '../internal/data';
import { buttonRecipe as r } from './Button.recipe';
import { LayerContext } from '../internal/layer';
import { SharedRowContext } from '../internal/footer';

export type ButtonVariant = 'primary' | 'secondary' | 'tonal' | 'ghost' | 'link' | 'destructive';
export type ButtonSize = 'sm' | 'md' | 'lg';
export type ButtonWidth = 'content' | 'fill';
export type ButtonEdge = 'start' | 'end';

export interface ButtonOwnProps {
  /** The visible text and the accessible name. Name the outcome: "Save changes", not "OK". */
  label: string;
  /**
   * In a ButtonGroup the group's variant, unless set here. `primary` for the one main action in view, `tonal` for a secondary action inside a card or row, filled without an edge, `destructive` for an action that loses data, `link`
   * for a quiet way out beside a form's main action ("Forgot password?").
   * @default 'secondary'
   */
  variant?: ButtonVariant;
  /** In a ButtonGroup the group's size, unless set here. @default 'md' */
  size?: ButtonSize;
  /** An icon before the label, by name or as a definition; hidden from assistive tech, since the label names the button. */
  startIcon?: IconRef;
  /** An icon after the label: a chevron for a button that opens something, an arrow for one that goes on. */
  endIcon?: IconRef;
  /** Draws `startIcon` alone in a square; `label` stays the accessible name. For a named icon button use IconButton. @default false */
  isIconOnly?: boolean;
  /** Drawn after the label: a count, a shortcut hint. Hidden from assistive tech. */
  endContent?: Child;
  /** Lifts the button off the page with a shadow, for the one action that floats over scrolling content. @default false */
  isFloating?: boolean;
  /** `content` keeps its own width in a column; `fill` stretches across. Unset follows the parent. */
  width?: ButtonWidth;
  /** For a ghost or link button at the edge of a row: its padding hangs past that side, so the label or the icon's drawing lines up with the content edge, as the text above and below does. */
  edge?: ButtonEdge;
  /** A press leaves the focused editor editing, its caret, selection and keyboard kept: for a format bar. @default false */
  isFocusKept?: boolean;
  /** @default false */
  isDisabled?: boolean;
  /** Why it is disabled: read as the button's description while disabled; not drawn. */
  disabledMessage?: string;
  /** Shows a spinner and ignores presses; the button stays focusable and reports busy. @default false */
  isLoading?: boolean;
  /** Merged last, so its classes win over the recipe's. */
  className?: string;
  'data-testid'?: string;
}

/** What a press on a Button or IconButton does. */
export interface ButtonActions {
  /** Runs on press, before `clickAction`. */
  onClick?: () => void;
  /** An async action, such as a Logic action. The button shows loading until it settles; a rejection is logged with the label. */
  clickAction?: () => Promise<unknown>;
}

export type ButtonProps = ButtonOwnProps & Press<ButtonActions>;

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
/** The glyph's side in points at the default text size; it grows with the text, up to half again. */
const glyphSides = { sm: 16, md: 18, lg: 20 } satisfies Record<ButtonSize, number>;
/** An icon-only button's square in points, as `size-control-*`. */
const squareSides = { sm: 32, md: 40, lg: 48 } satisfies Record<ButtonSize, number>;

export function Button({
  label,
  isFocusKept,
  variant: ownVariant,
  size: ownSize,
  startIcon,
  endIcon,
  isIconOnly = false,
  endContent,
  isFloating = false,
  width,
  edge: ownEdge,
  isDisabled: ownDisabled = false,
  disabledMessage,
  isLoading = false,
  onClick,
  clickAction,
  to,
  className,
  'data-testid': testId, ...attributes
}: ButtonProps) {
  const group = useContext(ButtonGroupContext);
  const shares = useContext(SharedRowContext) && width === undefined && !isIconOnly && group === null;
  const layer = useContext(LayerContext);
  const trigger = useTrigger();
  const variant = ownVariant ?? group?.variant ?? 'secondary';
  const size = ownSize ?? group?.size ?? 'md';
  // A link has no chrome of its own, so where a part sets an edge its text lines up with the content.
  const placedEdge = useContext(ActionEdge);
  const flush = ownEdge === undefined && variant === 'link' && placedEdge !== null ? placedEdge : undefined;
  const edge = ownEdge;
  // An icon-only square grows by what its glyph grows, keeping its padding; so does a hanging edge.
  const glyph = useTextGrowth(glyphSides[size], glyphGrowth);
  const square = isIconOnly && glyph ? squareSides[size] + glyph.width - glyphSides[size] : undefined;
  const squareStyle = square === undefined ? undefined : {
    width: square, height: square,
    ...(edge && { [edge === 'start' ? 'marginInlineStart' : 'marginInlineEnd']: -((square - glyph!.width) / 2 + glyph!.width * 5 / 24) }),
  };
  const isDisabled = ownDisabled || group?.isDisabled === true;
  const reasonId = `${useId()}-reason`;
  const hasReason = isDisabled && disabledMessage !== undefined;
  const pending = useLocal('pending', false);
  const busy = isLoading || pending.value;
  const press = async () => {
    if (busy) return;
    onClick?.();
    if (!clickAction) return;
    pending.set(true);
    try {
      await clickAction();
    } catch (error) {
      // Logged here, in every build: nothing else would show that the action failed.
      reportFailure(isIconOnly ? 'IconButton' : 'Button', label, error);
    } finally {
      settleFlag(pending, false);
    }
  };
  const classes = cx(
    r.root.base, r.root.variant[variant], variant === 'secondary' && layer === 'overlay' && r.root.overlay, r.root.disabled[variant], isIconOnly ? r.root.square[size] : r.root.size[size],
    flush && r.root.flush[flush], group?.isAttached && r.root.attached, isFloating && !isDisabled && r.root.floating, width && r.root.width[width], shares && r.root.share, edge && r.root.edge[edge][isIconOnly ? 'square' : (edge === 'end' ? endIcon : startIcon) ? 'icon' : 'label'][size], className,
  );
  const content = (
    <>
      {busy ? (
        <SpinnerMark ring={spinnerRing[variant]} className={cx(r.spinner.base, r.spinner.size[size])} />
      ) : (
        startIcon !== undefined && <Glyph name={startIcon} side={glyphSides[size]} className={r.glyph.size[size]} />
      )}
      {/* An icon-only button keeps its label as the name; nothing else is drawn beside the icon. */}
      {!isIconOnly && <text className={cx(r.label.base, r.label.weight[variant], r.label.size[size])}>{label}</text>}
      {!isIconOnly && endIcon !== undefined && <Glyph name={endIcon} side={glyphSides[size]} className={r.glyph.size[size]} />}
      {!isIconOnly && endContent !== undefined && <view aria-hidden className={r.icon.base}>{endContent}</view>}
      {hasReason && <text id={reasonId} aria-hidden className={r.reason.base}>{disabledMessage}</text>}
    </>
  );
  if (to !== undefined) {
    if (onClick !== undefined || clickAction !== undefined || trigger) throw Error(`Button "${label}": give to or an action, not both`);
    return (
      <link to={to} data-testid={testId} {...dataOf(attributes)} aria-label={label} aria-describedby={hasReason ? reasonId : undefined} disabled={isDisabled} style={squareStyle} className={classes}>
        {content}
      </link>
    );
  }
  return (
    <button
      {...identityOf(testId, attributes, trigger)}
      aria-label={label}
      aria-describedby={hasReason ? reasonId : trigger?.describedBy}
      aria-busy={busy || undefined}
      disabled={isDisabled}
      keepsFocus={isFocusKept || undefined}
      popoverTarget={trigger?.popoverTarget}
      interestFor={trigger?.interestFor}
      onClick={press}
      style={squareStyle}
      className={classes}
    >
      {content}
    </button>
  );
}
registerPressable(Button);
