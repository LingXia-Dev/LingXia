import { cx, useLocal, type BoxSize, type Child } from '@lingxia/view';
import { ButtonGroupContext, type ButtonSize, type ButtonVariant } from '../Button/Button';
import { buttonGroupRecipe as r } from './ButtonGroup.recipe';
import { dataOf } from '../internal/data';

export interface ButtonGroupProps {
  /** What the buttons have in common, read once as the group's name: "Text style". */
  label: string;
  /** Button and IconButton elements. */
  children: Child;
  /** Joins the buttons edge to edge, for actions of one kind. A row too long for one line comes apart and wraps, each button whole, as apart ones do. @default false */
  isAttached?: boolean;
  /** The size of every button that sets none. */
  size?: ButtonSize;
  /** The variant of every button that sets none. */
  variant?: ButtonVariant;
  /** Disables every button in the group. @default false */
  isDisabled?: boolean;
  className?: string;
  'data-testid'?: string;
}

/** How many elements `children` holds, through arrays. */
function count(children: unknown): number {
  if (Array.isArray(children)) return children.reduce((sum: number, child) => sum + count(child), 0);
  return children === null || children === undefined || typeof children === 'boolean' ? 0 : 1;
}

/** Buttons that belong together, as one stop for the keyboard and one named group for a screen reader. */
export function ButtonGroup({ label, children, isAttached = false, size, variant, isDisabled = false, className, 'data-testid': testId, ...attributes }: ButtonGroupProps) {
  // Joined edges wrapped onto two lines leave square corners at the break, so an attached row
  // too long for one line lays out apart. Apart, each join holds an 8 pt gap, not a 1 pt overlap.
  const fits = useLocal('fits', true);
  const isJoined = isAttached && fits.value;
  const joins = count(children) - 1;
  const measure = (box: BoxSize) => {
    const oneLine = box.maxContentWidth - (isJoined ? 0 : joins * 9);
    const next = oneLine <= box.width + 0.5;
    if (next !== fits.value) fits.set(next);
  };
  return (
    <ButtonGroupContext.Provider value={{ isAttached: isJoined, size, variant, isDisabled }}>
      <view onLayout={isAttached ? measure : undefined} role="toolbar" aria-label={label} data-testid={testId} {...dataOf(attributes)} className={cx(r.root.base, isJoined ? r.root.attached : r.root.apart, className)}>
        {children}
      </view>
    </ButtonGroupContext.Provider>
  );
}
