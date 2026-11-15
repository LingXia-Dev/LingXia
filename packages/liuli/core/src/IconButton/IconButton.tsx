import { cx, useContext } from '@lingxia/view';
import { Button, ButtonGroupContext, type ButtonSize, type ButtonVariant } from '../Button/Button';
import type { IconName } from '../Icon/Icon';
import { iconButtonRecipe as r } from './IconButton.recipe';

export interface IconButtonProps {
  /** The accessible name: what pressing it does ("Delete note"). Not drawn, so it is never optional. */
  label: string;
  /** The icon, by name. */
  icon: IconName;
  /** In a ButtonGroup the group's variant, unless set here. Quiet by default: an icon button is rarely the main action. @default 'ghost' */
  variant?: ButtonVariant;
  /** 32, 40 or 48 points square. In a ButtonGroup the group's size, unless set here. @default 'md' */
  size?: ButtonSize;
  /** @default false */
  isDisabled?: boolean;
  /** Why it is disabled: read as its description while disabled; not drawn. */
  disabledMessage?: string;
  /** Shows a spinner in place of the icon and ignores presses. @default false */
  isLoading?: boolean;
  onClick?: () => void;
  /** An async action; the button shows loading until it settles. */
  clickAction?: () => Promise<unknown>;
  className?: string;
  'data-testid'?: string;
}

/** A button that shows only an icon, with a name every assistive technology reads. */
export function IconButton({ label, icon, variant, size, className, ...rest }: IconButtonProps) {
  // Quiet on its own; inside a group that sets a variant, the group's.
  const grouped = useContext(ButtonGroupContext)?.variant;
  return <Button {...rest} label={label} startIcon={icon} isIconOnly variant={variant ?? (grouped ? undefined : 'ghost')} size={size} className={cx(r.root.base, className)} />;
}
