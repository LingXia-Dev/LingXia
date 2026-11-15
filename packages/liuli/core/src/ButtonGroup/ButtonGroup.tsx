import { cx, type Child } from '@lingxia/view';
import { ButtonGroupContext, type ButtonSize, type ButtonVariant } from '../Button/Button';
import { buttonGroupRecipe as r } from './ButtonGroup.recipe';

export interface ButtonGroupProps {
  /** What the buttons have in common, read once as the group's name: "Text style". */
  label: string;
  /** Button and IconButton elements. */
  children: Child;
  /** Joins the buttons edge to edge, for actions of one kind. Apart, they keep a gap and wrap. @default false */
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

/** Buttons that belong together, as one stop for the keyboard and one named group for a screen reader. */
export function ButtonGroup({ label, children, isAttached = false, size, variant, isDisabled = false, className, 'data-testid': testId }: ButtonGroupProps) {
  return (
    <ButtonGroupContext.Provider value={{ isAttached, size, variant, isDisabled }}>
      <view role="toolbar" aria-label={label} data-testid={testId} className={cx(r.root.base, isAttached ? r.root.attached : r.root.apart, className)}>
        {children}
      </view>
    </ButtonGroupContext.Provider>
  );
}
