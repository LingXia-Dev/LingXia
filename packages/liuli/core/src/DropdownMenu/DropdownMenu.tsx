import { createContext, cx, useContext, useId, type Child } from '@lingxia/view';
import type { ButtonSize, ButtonVariant } from '../Button/Button';
import { buttonRecipe as b } from '../Button/Button.recipe';
import { Glyph } from '../internal/Glyph';
import { dropdownMenuRecipe as r } from './DropdownMenu.recipe';

export type DropdownMenuItemVariant = 'default' | 'destructive';
export type DropdownMenuPlacement = 'bottom' | 'top';
export type DropdownMenuAlignment = 'start' | 'center' | 'end';

export interface DropdownMenuProps {
  /** The trigger's text, the trigger's accessible name and the menu's accessible name. */
  label: string;
  /**
   * For a menu that sets a value: the current choice, drawn on the trigger in place of `label` and
   * read after it as the trigger's description (label "Appearance", value "System").
   */
  value?: string;
  /** Draws only `icon` on a square trigger; `label` stays the name. @default false */
  isLabelHidden?: boolean;
  /** An icon on the trigger, before the label. */
  icon?: Child;
  /** A down chevron after the label. Defaults to shown unless the label is hidden. */
  hasChevron?: boolean;
  /** The trigger's look, from Button. @default 'secondary' */
  variant?: ButtonVariant;
  /** The trigger's and the items' height. @default 'md' */
  size?: ButtonSize;
  /** `DropdownMenuItem`s and `DropdownMenuDivider`s, in order. */
  children: Child;
  /**
   * Controls whether the menu is open; leave it out to let the trigger own it. Opened from code,
   * the menu is placed by its classes, not under the trigger.
   */
  isOpen?: boolean;
  onOpenChange?: (isOpen: boolean) => void;
  /** Which side of the trigger the menu opens on. @default 'bottom' */
  placement?: DropdownMenuPlacement;
  /** Which edge of the trigger the menu lines up with. @default 'start' */
  alignment?: DropdownMenuAlignment;
  /** @default false */
  isDisabled?: boolean;
  /** Classes for the trigger. */
  className?: string;
  /** Classes for the menu panel. */
  menuClassName?: string;
  /** On the trigger. */
  'data-testid'?: string;
}

export interface DropdownMenuItemProps {
  /** The item's text and its accessible name. */
  label: string;
  /** A second line, read as the item's description. */
  description?: string;
  /** Drawn before the label; hidden from assistive tech. */
  icon?: Child;
  /** Runs on choice; the menu then closes. */
  onClick: () => void;
  /** @default false */
  isDisabled?: boolean;
  /** `destructive` tints the icon with the danger role; the text stays readable. @default 'default' */
  variant?: DropdownMenuItemVariant;
  'data-testid'?: string;
}

export interface DropdownMenuCheckboxItemProps {
  /** The item's text and its accessible name. */
  label: string;
  /** A second line, read as the item's description. */
  description?: string;
  isChecked: boolean;
  /** Runs with the new state; the menu stays open. */
  onCheckedChange: (isChecked: boolean) => void;
  /** @default false */
  isDisabled?: boolean;
  'data-testid'?: string;
}

export interface DropdownMenuRadioGroupProps {
  /** The checked item's value; `null` while none is. */
  value: string | null;
  /** Runs with the value of the item chosen; the menu stays open. */
  onChange: (value: string) => void;
  /** `DropdownMenuRadioItem`s. */
  children: Child;
  /** Names the choices for assistive input when the menu's label does not, as in a menu with other items. */
  label?: string;
}

export interface DropdownMenuRadioItemProps {
  /** What the group's `onChange` carries. */
  value: string;
  /** The item's text and its accessible name. */
  label: string;
  /** A second line, read as the item's description. */
  description?: string;
  /** @default false */
  isDisabled?: boolean;
  'data-testid'?: string;
}

// `span-inline-end` spans from the trigger's start edge toward the inline end, so the menu's start
// edge lines up with the trigger's; logical, so it mirrors in RTL.
const areas = {
  bottom: { start: 'bottom span-inline-end', center: 'bottom', end: 'bottom span-inline-start' },
  top: { start: 'top span-inline-end', center: 'top', end: 'top span-inline-start' },
} as const satisfies Record<DropdownMenuPlacement, Record<DropdownMenuAlignment, string>>;

/** The items' height; set by whatever holds them: a DropdownMenu or a ContextMenu. */
export const MenuSize = createContext<ButtonSize | null>(null);
/** Set by a holder that does not close by itself, such as a sheet: an item calls it after its action. */
export const MenuClose = createContext<(() => void) | null>(null);
const RadioGroup = createContext<DropdownMenuRadioGroupProps | null>(null);

/**
 * A button that opens a menu of actions. The menu has the View's menu keyboard: arrows, Home, End
 * and type-ahead, opened from the keyboard on its first item.
 */
export function DropdownMenu({
  label,
  value,
  isLabelHidden = false,
  icon,
  hasChevron = !isLabelHidden,
  variant = 'secondary',
  size = 'md',
  children,
  isOpen,
  onOpenChange,
  placement = 'bottom',
  alignment = 'start',
  isDisabled = false,
  className,
  menuClassName,
  'data-testid': testId,
}: DropdownMenuProps) {
  const id = useId();
  const valueId = `${id}-value`;
  return (
    <>
      <button
        popoverTarget={id}
        aria-label={label}
        aria-describedby={value !== undefined && !isLabelHidden ? valueId : undefined}
        disabled={isDisabled}
        data-testid={testId}
        className={cx(b.root.base, b.root.variant[variant], b.root.disabled[variant], isLabelHidden ? b.root.square[size] : b.root.size[size], className)}
      >
        {icon !== undefined && <view aria-hidden className={b.icon.base}>{icon}</view>}
        {!isLabelHidden && <text id={value !== undefined ? valueId : undefined} className={cx(b.label.base, b.label.weight[variant], b.label.size[size])}>{value ?? label}</text>}
        {hasChevron && <Glyph name="chevron-down" className={r.chevron.base} />}
      </button>
      <MenuSize.Provider value={size}>
        <view
          id={id}
          popover
          role="menu"
          aria-label={label}
          open={isOpen}
          onToggle={onOpenChange}
          style={{ positionArea: areas[placement][alignment] }}
          className={cx(r.menu.base, r.menu.offset[placement], menuClassName)}
        >
          {children}
        </view>
      </MenuSize.Provider>
    </>
  );
}

/** One action in a DropdownMenu. */
export function DropdownMenuItem({
  label,
  description,
  icon,
  onClick,
  isDisabled = false,
  variant = 'default',
  'data-testid': testId,
}: DropdownMenuItemProps) {
  const size = useContext(MenuSize);
  const close = useContext(MenuClose);
  const descriptionId = `${useId()}-description`;
  if (size === null) throw Error('DropdownMenuItem: place it inside a DropdownMenu');
  return (
    <button
      role="menuitem"
      aria-label={label}
      aria-describedby={description !== undefined ? descriptionId : undefined}
      disabled={isDisabled}
      onClick={() => { onClick?.(); close?.(); }}
      data-testid={testId}
      className={cx(r.item.base, r.item.size[size])}
    >
      {icon !== undefined && <view aria-hidden className={cx(r.itemIcon.base, r.itemIcon.variant[variant])}>{icon}</view>}
      <view className={r.itemText.base}>
        <text className={r.itemLabel.base}>{label}</text>
        {description !== undefined && <text id={descriptionId} className={r.itemDescription.base}>{description}</text>}
      </view>
    </button>
  );
}

/** An item that turns one option on or off, checked at its end. */
export function DropdownMenuCheckboxItem({ label, description, isChecked, onCheckedChange, isDisabled = false, 'data-testid': testId }: DropdownMenuCheckboxItemProps) {
  return (
    <CheckableItem role="menuitemcheckbox" label={label} description={description} isChecked={isChecked} isDisabled={isDisabled}
      onClick={() => onCheckedChange(!isChecked)} testId={testId} owner="DropdownMenuCheckboxItem" />
  );
}

export interface DropdownMenuGroupProps {
  /** The group's heading, drawn over its items and read as the group's name: "Sort by". */
  label: string;
  /** `DropdownMenuItem`s or checkable items, in order. */
  children: Child;
}

/** Items under a heading. Put a DropdownMenuDivider between groups. */
export function DropdownMenuGroup({ label, children }: DropdownMenuGroupProps) {
  return (
    <view role="group" aria-label={label} className={r.group.base}>
      <text aria-hidden className={r.groupLabel.base}>{label}</text>
      {children}
    </view>
  );
}

/** DropdownMenuRadioItems of which one is checked. It draws nothing of its own; set it off with dividers. */
export function DropdownMenuRadioGroup(props: DropdownMenuRadioGroupProps) {
  return (
    <RadioGroup.Provider value={props}>
      <view role="group" aria-label={props.label} className={r.radioGroup.base}>{props.children}</view>
    </RadioGroup.Provider>
  );
}

/** One choice in a DropdownMenuRadioGroup, checked at its end while it is the group's value. */
export function DropdownMenuRadioItem({ value, label, description, isDisabled = false, 'data-testid': testId }: DropdownMenuRadioItemProps) {
  const group = useContext(RadioGroup);
  if (group === null) throw Error('DropdownMenuRadioItem: place it inside a DropdownMenuRadioGroup');
  return (
    <CheckableItem role="menuitemradio" label={label} description={description} isChecked={group.value === value} isDisabled={isDisabled}
      onClick={() => { if (group.value !== value) group.onChange(value); }} testId={testId} owner="DropdownMenuRadioItem" />
  );
}

function CheckableItem(props: {
  role: 'menuitemcheckbox' | 'menuitemradio';
  label: string;
  description: string | undefined;
  isChecked: boolean;
  isDisabled: boolean;
  onClick: () => void;
  testId: string | undefined;
  owner: string;
}) {
  const size = useContext(MenuSize);
  const descriptionId = `${useId()}-description`;
  if (size === null) throw Error(`${props.owner}: place it inside a DropdownMenu`);
  return (
    <button
      role={props.role}
      aria-checked={props.isChecked}
      aria-label={props.label}
      aria-describedby={props.description !== undefined ? descriptionId : undefined}
      disabled={props.isDisabled}
      onClick={props.onClick}
      data-testid={props.testId}
      className={cx(r.item.base, r.item.size[size])}
    >
      <view className={r.itemText.base}>
        <text className={r.itemLabel.base}>{props.label}</text>
        {props.description !== undefined && <text id={descriptionId} className={r.itemDescription.base}>{props.description}</text>}
      </view>
      {props.isChecked && <Glyph name="check" weight={2.5} className={r.itemCheck.base} />}
    </button>
  );
}

/** A hairline between groups of items; hidden from assistive tech. */
export function DropdownMenuDivider() {
  return <view aria-hidden className={r.divider.base} />;
}
