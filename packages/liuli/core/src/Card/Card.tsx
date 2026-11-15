import { createContext, cx, useContext, type Child, type Local } from '@lingxia/view';
import { cardRecipe as r } from './Card.recipe';

export type CardVariant = 'outlined' | 'raised' | 'floating' | 'sunken';
export type CardPadding = 'none' | 'sm' | 'md' | 'lg';

export interface CardProps {
  children: Child;
  /** `outlined` sits on the page, `raised` rests one step above it, `floating` hovers over what is behind it (a map, a picture), `sunken` groups content inside another surface. @default 'outlined' */
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

interface GroupState {
  chosen: string | null;
  isDisabled: boolean;
  choose: (value: string) => void;
}

const Group = createContext<GroupState | null>(null);

export type ClickableCardProps = CardProps & {
  /** The accessible name: a short summary of the card, since its content is not read separately. */
  label: string;
  /** @default false */
  isDisabled?: boolean;
} & (
    | {
        onClick: () => void;
        /** Marks the card as the chosen one among several; reported as selected. @default false */
        isSelected?: boolean;
        value?: never;
      }
    | {
        /** Inside a `ClickableCardGroup`: what the group's value carries when this card is chosen. */
        value: string;
        /** Runs after the group's choice. */
        onClick?: () => void;
        isSelected?: never;
      }
  );

/** A card that is one button as a whole. It must not contain other controls. */
export function ClickableCard({
  children,
  label,
  onClick,
  isSelected = false,
  isDisabled = false,
  value,
  variant = 'outlined',
  padding = 'md',
  className,
  'data-testid': testId,
}: ClickableCardProps) {
  const group = useContext(Group);
  const asRadio = group !== null && value !== undefined;
  const isChosen = asRadio ? group.chosen === value : isSelected;
  return (
    <button
      role={asRadio ? 'radio' : undefined}
      aria-checked={asRadio ? isChosen : undefined}
      aria-selected={!asRadio && isChosen ? true : undefined}
      aria-label={label}
      disabled={isDisabled || (asRadio && group.isDisabled)}
      onClick={() => {
        if (asRadio) group.choose(value);
        onClick?.();
      }}
      data-testid={testId}
      className={cx(r.root.base, r.root.variant[variant], r.root.padding[padding], r.root.pressable, isChosen && r.root.selected, className)}
    >
      {children}
    </button>
  );
}

export type ClickableCardGroupBinding =
  | {
      /** A View-local cell the group reads and writes; use it or `value` with `onChange`. */
      bind: Local<string | null>;
      value?: never;
      onChange?: never;
    }
  | {
      bind?: never;
      /** The chosen card's value; `null` while none is chosen. */
      value: string | null;
      /** Runs with the card chosen by a tap or by arrowing onto it. */
      onChange: (value: string) => void;
    };

export type ClickableCardGroupProps = ClickableCardGroupBinding & {
  /** Names the group for assistive input. */
  label: string;
  /** ClickableCards, each with a `value`. */
  children: Child;
  /** @default false */
  isDisabled?: boolean;
  /** Lays the cards out: a row or a column, and the gap. */
  className?: string;
  'data-testid'?: string;
};

/**
 * ClickableCards that choose one value, reported as a radio group: one Tab stop, and the arrows move
 * between cards and choose on arrival.
 */
export function ClickableCardGroup(props: ClickableCardGroupProps) {
  const { label, children, isDisabled = false, className, 'data-testid': testId } = props;
  const chosen = props.bind ? props.bind.value : props.value;
  const choose = (next: string) => {
    if (next === chosen) return;
    if (props.bind) props.bind.set(next);
    else props.onChange(next);
  };
  return (
    <Group.Provider value={{ chosen, isDisabled, choose }}>
      <view role="radiogroup" aria-label={label} data-testid={testId} className={cx(r.group.base, className)}>
        {children}
      </view>
    </Group.Provider>
  );
}
