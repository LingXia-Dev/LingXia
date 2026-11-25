import { cx, type Child } from '@lingxia/view';
import { liuliText } from '../i18n/text';
import type { Hue } from '../hue';
import type { IconRef } from '../Icon/Icon';
import { Glyph, glyphGrowth, useTextGrowth } from '../internal/Glyph';
import { tokenRecipe as r } from './Token.recipe';
import { dataOf } from '../internal/data';

export type TokenSize = 'sm' | 'md';

export type TokenRemove =
  | {
      /** Takes the token away; the app stops rendering it. */
      onRemove: () => void;
      /** The remove button's name, with what it removes: "Remove Ana Lima". Defaults to "Remove <label>" in the page's language. */
      removeLabel?: string;
    }
  | { onRemove?: never; removeLabel?: never };

export type TokenProps = TokenRemove & {
  /** What the token stands for: a person, a tag, a filter. */
  label: string;
  /** An icon before the label, by name or as a definition. */
  startIcon?: IconRef;
  /** A category's colour, one of the theme's eight hues: the pill takes the hue's soft fill, with a dot (or the icon) in the hue. */
  hue?: Hue;
  /** Makes the label a button: opens the thing, or toggles a filter. */
  onClick?: () => void;
  /** Marks a pressable token as on, for a filter that is applied. @default false */
  isSelected?: boolean;
  /** @default 'md' */
  size?: TokenSize;
  /** @default false */
  isDisabled?: boolean;
  className?: string;
  /** On the token; the remove button gets `<testid>-remove`. */
  'data-testid'?: string;
};

/** Glyph sides and the remove target in points at the default text size, as the recipe's `size-*`. */
const iconSides = { sm: 14, md: 16 } satisfies Record<TokenSize, number>;
const removeSides = { sm: 12, md: 14 } satisfies Record<TokenSize, number>;
const removeTargets = { sm: 24, md: 32 } satisfies Record<TokenSize, number>;

/** One chosen thing as a pill: a recipient, a tag, a filter that can be taken away. */
export function Token({ label, startIcon, hue, onClick, isSelected = false, size = 'md', isDisabled = false, onRemove, removeLabel, className, 'data-testid': testId, ...attributes }: TokenProps) {
  // The remove target grows by what its glyph grows, as an icon-only button does.
  const cross = useTextGrowth(removeSides[size], glyphGrowth);
  const removeSide = cross && removeTargets[size] + cross.width - removeSides[size];
  const content = (
    <>
      {startIcon !== undefined && <Glyph name={startIcon} side={iconSides[size]} className={cx(r.icon.size[size], hue !== undefined && !isDisabled && r.icon.hue[hue])} />}
      {startIcon === undefined && hue !== undefined && <view aria-hidden className={cx(r.dot.base, isDisabled ? r.dot.disabled : r.dot.hue[hue])} />}
      <text className={cx(r.label.base, r.label.size[size])}>{label}</text>
    </>
  );
  const remove = onRemove !== undefined && (
    <button aria-label={removeLabel ?? liuliText('@liuli.removeNamed', { name: label })} disabled={isDisabled} onClick={onRemove} data-testid={testId && `${testId}-remove`} style={removeSide ? { width: removeSide, height: removeSide } : undefined} className={cx(r.remove.base, r.remove.size[size])}>
      <Glyph name="close" weight={2.5} side={removeSides[size]} className={r.removeIcon.size[size]} />
    </button>
  );
  const classes = cx(r.root.base, r.root.size[size], onRemove === undefined && r.root.plain[size], !isDisabled && hue !== undefined && r.root.hue[hue], !isDisabled && isSelected && r.root.selected, isDisabled && r.root.disabled, className);
  // A token that both opens and can be removed is two buttons side by side, never one inside the other.
  if (onClick !== undefined && onRemove === undefined) {
    return (
      <button aria-label={label} aria-pressed={isSelected} disabled={isDisabled} onClick={onClick} data-testid={testId} {...dataOf(attributes)} className={cx(classes, r.root.pressable)}>
        {content}
      </button>
    );
  }
  return (
    <view data-testid={testId} {...dataOf(attributes)} className={classes}>
      {onClick !== undefined
        ? <button aria-label={label} disabled={isDisabled} onClick={onClick} className={cx(r.open.base, r.open.size[size])}>{content}</button>
        : content}
      {remove}
    </view>
  );
}

export interface TokenListProps {
  /** Names the tokens for assistive tech: "Recipients". */
  label: string;
  /** `Token`s, in order. */
  children: Child;
  className?: string;
  'data-testid'?: string;
}

/** Tokens in a row that wraps, read as a named list. */
export function TokenList({ label, children, className, 'data-testid': testId, ...attributes }: TokenListProps) {
  return <view role="group" aria-label={label} data-testid={testId} {...dataOf(attributes)} className={cx(r.list.base, className)}>{children}</view>;
}
