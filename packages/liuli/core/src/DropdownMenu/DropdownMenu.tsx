import { createContext, cx, useContext, useId, type Child } from '@lingxia/view';
import type { ButtonSize, ButtonVariant } from '../Button/Button';
import { buttonRecipe as b } from '../Button/Button.recipe';
import { glyphBox, glyphs } from '../internal/glyphs';
import { dropdownMenuRecipe as r } from './DropdownMenu.recipe';

export type DropdownMenuItemVariant = 'default' | 'destructive';
export type DropdownMenuPlacement = 'bottom' | 'top';
export type DropdownMenuAlignment = 'start' | 'center' | 'end';

export interface DropdownMenuProps {
  /** The trigger's text, the trigger's accessible name and the menu's accessible name. */
  label: string;
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
  /** The item's text and the start of its accessible name. */
  label: string;
  /** A second line, read after the label. */
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

// `span-inline-end` spans from the trigger's start edge toward the inline end, so the menu's start
// edge lines up with the trigger's; logical, so it mirrors in RTL.
const areas = {
  bottom: { start: 'bottom span-inline-end', center: 'bottom', end: 'bottom span-inline-start' },
  top: { start: 'top span-inline-end', center: 'top', end: 'top span-inline-start' },
} as const satisfies Record<DropdownMenuPlacement, Record<DropdownMenuAlignment, string>>;

const MenuSize = createContext<ButtonSize | null>(null);

/**
 * A button that opens a menu of actions. The menu has the View's menu keyboard: arrows, Home, End
 * and type-ahead, opened from the keyboard on its first item.
 */
export function DropdownMenu({
  label,
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
  return (
    <>
      <button
        popoverTarget={id}
        aria-label={label}
        disabled={isDisabled}
        data-testid={testId}
        className={cx(b.root.base, b.root.variant[variant], isLabelHidden ? b.root.square[size] : b.root.size[size], className)}
      >
        {icon !== undefined && <view aria-hidden className={b.icon.base}>{icon}</view>}
        {!isLabelHidden && <text className={cx(b.label.base, b.label.size[size])}>{label}</text>}
        {hasChevron && <vector aria-hidden paths={glyphs.chevronDown} viewBox={glyphBox} className={r.chevron.base} />}
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
  if (size === null) throw Error('DropdownMenuItem: place it inside a DropdownMenu');
  return (
    <button
      role="menuitem"
      disabled={isDisabled}
      onClick={onClick}
      data-testid={testId}
      className={cx(r.item.base, r.item.size[size])}
    >
      {icon !== undefined && <view aria-hidden className={cx(r.itemIcon.base, r.itemIcon.variant[variant])}>{icon}</view>}
      <view className={r.itemText.base}>
        <text className={r.itemLabel.base}>{label}</text>
        {description !== undefined && <text className={r.itemDescription.base}>{description}</text>}
      </view>
    </button>
  );
}

/** A hairline between groups of items; hidden from assistive tech. */
export function DropdownMenuDivider() {
  return <view aria-hidden className={r.divider.base} />;
}
