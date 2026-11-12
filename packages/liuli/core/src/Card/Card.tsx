import { cx, type Child } from '@lingxia/view';
import { cardRecipe as r } from './Card.recipe';

export type CardVariant = 'outlined' | 'raised' | 'sunken';
export type CardPadding = 'none' | 'sm' | 'md' | 'lg';

export interface CardProps {
  children: Child;
  /** `outlined` sits on the page, `raised` floats above it, `sunken` groups content inside another surface. @default 'outlined' */
  variant?: CardVariant;
  /** The inset and the gap between children. @default 'md' */
  padding?: CardPadding;
  className?: string;
  'data-testid'?: string;
}

/** A surface that groups related content. */
export function Card({ children, variant = 'outlined', padding = 'md', className, 'data-testid': testId }: CardProps) {
  return (
    <view data-testid={testId} className={cx(r.root.base, r.root.variant[variant], r.root.padding[padding], className)}>
      {children}
    </view>
  );
}

export interface ClickableCardProps extends CardProps {
  /** The accessible name: a short summary of the card, since its content is not read separately. */
  label: string;
  onClick: () => void;
  /** Marks the card as the chosen one among several; reported as selected. @default false */
  isSelected?: boolean;
  /** @default false */
  isDisabled?: boolean;
}

/** A card that is one button as a whole. It must not contain other controls. */
export function ClickableCard({
  children,
  label,
  onClick,
  isSelected = false,
  isDisabled = false,
  variant = 'outlined',
  padding = 'md',
  className,
  'data-testid': testId,
}: ClickableCardProps) {
  return (
    <button
      aria-label={label}
      aria-selected={isSelected || undefined}
      disabled={isDisabled}
      onClick={onClick}
      data-testid={testId}
      className={cx(r.root.base, r.root.variant[variant], r.root.padding[padding], r.root.pressable, isSelected && r.root.selected, className)}
    >
      {children}
    </button>
  );
}
