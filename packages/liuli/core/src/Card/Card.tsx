import { createContext, cx, useContext, useId, type Child, type Local } from '@lingxia/view';
import { cardRecipe as r } from './Card.recipe';
import { Glyph } from '../internal/Glyph';
import { dataOf } from '../internal/data';
import type { Press } from '../internal/press';

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
export function Card({ children, variant = 'outlined', padding = 'md', className, 'data-testid': testId, ...attributes }: CardProps) {
  return (
    <view data-testid={testId} {...dataOf(attributes)} className={cx(r.root.base, r.root.variant[variant], r.root.padding[padding], className)}>
      {children}
    </view>
  );
}

interface GroupState {
  isMultiple: boolean;
  isOn: (value: string) => boolean;
  isDisabled: boolean;
  choose: (value: string) => void;
}

const Group = createContext<GroupState | null>(null);

export type ClickableCardProps = CardProps & {
  /** The accessible name: a short summary of the card, since its content is not read separately. */
  label: string;
  /** Read after the name, not drawn: the detail the card shows that the name leaves out, such as a price or a count. */
  description?: string;
  /** @default false */
  isDisabled?: boolean;
} & (
    | (Press<{
        onClick: () => void;
        /** Marks the card as chosen; reported as pressed, so a press turns it on and off. @default false */
        isSelected?: boolean;
      }> & { value?: never })
    | {
        /** Inside a `ClickableCardGroup`: what the group's value carries when this card is chosen. */
        value: string;
        /** Runs after the group's choice. */
        onClick?: () => void;
        isSelected?: never;
        to?: never;
      }
  );

/** A card that is one button as a whole, or one link with `to`. A button card must not contain other controls. */
export function ClickableCard({
  children,
  label,
  description,
  onClick,
  to,
  isSelected = false,
  isDisabled = false,
  value,
  variant = 'outlined',
  padding = 'md',
  className,
  'data-testid': testId, ...attributes
}: ClickableCardProps) {
  const group = useContext(Group);
  const inGroup = group !== null && value !== undefined;
  const asRadio = inGroup && !group.isMultiple;
  const isChosen = inGroup ? group.isOn(value) : isSelected;
  const descriptionId = `${useId()}-description`;
  const classes = cx(r.root.base, r.root.variant[variant], r.root.padding[padding], r.root.pressable, isChosen && r.root.selected, inGroup && group.isMultiple && r.root.checkRoom, className);
  if (to !== undefined) {
    return <link to={to} aria-label={label} aria-describedby={description !== undefined ? descriptionId : undefined} disabled={isDisabled} data-testid={testId} {...dataOf(attributes)} className={classes}>{children}{description !== undefined && <text id={descriptionId} aria-hidden className={r.description.base}>{description}</text>}</link>;
  }
  return (
    <button
      role={asRadio ? 'radio' : undefined}
      aria-checked={asRadio ? isChosen : undefined}
      aria-pressed={asRadio ? undefined : isChosen}
      aria-label={label}
      aria-describedby={description !== undefined ? descriptionId : undefined}
      disabled={isDisabled || (inGroup && group.isDisabled)}
      onClick={() => {
        if (inGroup) group.choose(value);
        onClick?.();
      }}
      data-testid={testId} {...dataOf(attributes)}
      className={classes}
    >
      {children}
      {description !== undefined && <text id={descriptionId} aria-hidden className={r.description.base}>{description}</text>}
      {/* Several may be chosen: a check in the corner says so, and shows each one's state beside the border. */}
      {inGroup && group.isMultiple && (
        <view aria-hidden className={cx(r.check.base, isChosen ? r.check.on : r.check.off)}>
          {isChosen && <Glyph name="check" side={14} className={r.checkGlyph.base} />}
        </view>
      )}
    </button>
  );
}

export type ClickableCardGroupBinding =
  | {
      /** One card at a time, read as a radio group. @default 'single' */
      selectionMode?: 'single';
      /** A View-local cell the group reads and writes; use it or `value` with `onChange`. */
      bind: Local<string | null>;
      value?: never;
      onChange?: never;
    }
  | {
      selectionMode?: 'single';
      bind?: never;
      /** The chosen card's value; `null` while none is chosen. */
      value: string | null;
      /** Runs with the card chosen by a tap or by arrowing onto it. */
      onChange: (value: string) => void;
    }
  | {
      /** Any number of cards, each read as pressed and marked with a check. */
      selectionMode: 'multiple';
      bind: Local<readonly string[]>;
      value?: never;
      onChange?: never;
    }
  | {
      selectionMode: 'multiple';
      bind?: never;
      /** The chosen cards' values. */
      value: readonly string[];
      /** Runs with the chosen values after a press, in the cards' order of choosing. */
      onChange: (value: readonly string[]) => void;
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
 * ClickableCards that choose a value. One at a time is a radio group: one Tab stop, and the arrows
 * move between cards and choose on arrival. With `selectionMode="multiple"` each card turns on and
 * off on its own, read as pressed, with a check in its corner.
 */
export function ClickableCardGroup(props: ClickableCardGroupProps) {
  const { label, children, isDisabled = false, className, 'data-testid': testId } = props;
  const state: GroupState = props.selectionMode === 'multiple'
    ? (() => {
        const on = props.bind ? props.bind.value : props.value;
        const set = (next: readonly string[]) => (props.bind ? props.bind.set(next) : props.onChange(next));
        return { isMultiple: true, isDisabled, isOn: value => on.includes(value), choose: value => set(on.includes(value) ? on.filter(each => each !== value) : [...on, value]) };
      })()
    : (() => {
        const chosen = props.bind ? props.bind.value : props.value;
        const set = (next: string) => (props.bind ? props.bind.set(next) : props.onChange(next));
        return { isMultiple: false, isDisabled, isOn: value => chosen === value, choose: value => { if (value !== chosen) set(value); } };
      })();
  return (
    <Group.Provider value={state}>
      <view role={state.isMultiple ? 'group' : 'radiogroup'} aria-label={label} data-testid={testId} {...dataOf(props)} className={cx(r.group.base, className)}>
        {children}
      </view>
    </Group.Provider>
  );
}
