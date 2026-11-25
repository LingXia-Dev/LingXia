import { createContext, cx, useContext, useId, useLocal, useView, type Child, type LinkTarget } from '@lingxia/view';
import type { ButtonSize } from '../Button/Button';
import type { IconRef } from '../Icon/Icon';
import { liuliText } from '../i18n/text';
import { Glyph } from '../internal/Glyph';
import { Trigger } from '../internal/trigger';
import { dropdownMenuRecipe as r } from './DropdownMenu.recipe';
import { dataOf } from '../internal/data';
import type { Press } from '../internal/press';
import { warnOnce } from '../internal/warn';

export type DropdownMenuItemVariant = 'default' | 'destructive';
export type DropdownMenuPlacement = 'bottom' | 'top';
export type DropdownMenuAlignment = 'start' | 'center' | 'end';

export interface DropdownMenuProps {
  /** The trigger: a Button or an IconButton, or any element, which is then wrapped in a plain button. */
  children: Child;
  /** `DropdownMenuItem`s, dividers, groups and checkable items, in order. Give this or `items`, not both. */
  menu?: Child;
  /** The rows as data: actions, dividers, groups and submenus. Checkable items are composed in `menu`. */
  items?: readonly DropdownMenuEntry[];
  /** The menu's accessible name. */
  label: string;
  /** The items' height; match the trigger's size. @default 'md' */
  size?: ButtonSize;
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
  /** Classes for the menu panel. */
  menuClassName?: string;
  /** On the trigger: the wrapper, or a Button or Link child without a test id of its own. The menu gets `<testid>-menu`. */
  'data-testid'?: string;
}

export type DropdownMenuItemProps = Press<{
  /** Runs on choice; the menu then closes, unless `isRepeatable`. */
  onClick: () => void;
}> & {
  /** A row pressed again and again, "Zoom in": it runs `onClick` and leaves the menu open. Not with `to`. @default false */
  isRepeatable?: boolean;
  /** The item's text and its accessible name. */
  label: string;
  /** A second line, read as the item's description. */
  description?: string;
  /** An icon by name from the set, or a definition, before the label; hidden from assistive tech. */
  icon?: IconRef;
  /** At the row's trailing edge, muted: a shortcut as a `Kbd`, or a count. Not read; name a shortcut in `description` if it matters. */
  endContent?: Child;
  /** @default false */
  isDisabled?: boolean;
  /** `destructive` draws the label and the icon in the danger role. @default 'default' */
  variant?: DropdownMenuItemVariant;
  'data-testid'?: string;
};

export interface DropdownMenuCheckboxItemProps {
  /** The item's text and its accessible name. */
  label: string;
  /** A second line, read as the item's description. */
  description?: string;
  value: boolean;
  /** Stands for a set that is partly on, such as "All labels": draws a dash and is read as mixed. A press asks for all. @default false */
  isIndeterminate?: boolean;
  /** Runs with the new state; the menu stays open. */
  onChange: (isChecked: boolean) => void;
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

/** One action, or a submenu when it has `items`, in a menu given as data. */
export interface DropdownMenuItemData {
  /** The row's own name: keys it, so it keeps its focus when the list reorders or grows; unique within its list. @default its place in the list */
  id?: string;
  /** The row's text and its accessible name. */
  label: string;
  /** A second line, read as the row's description. Not on a submenu. */
  description?: string;
  /** An icon by name from the set, or a definition, before the label; hidden from assistive tech. */
  icon?: IconRef;
  /** Runs on choice, with nothing; the menu then closes, unless `isRepeatable`. Leave it out on a submenu. */
  onClick?: () => void;
  /** A row pressed again and again, "Zoom in": it runs `onClick` and leaves the menu open. Not with `to` or `items`. @default false */
  isRepeatable?: boolean;
  /** Goes to a page instead of running `onClick`: `{ page, params }`, `{ tab }`, `{ back: 1 }`. Read as a menu item that is a link. Not on a submenu. */
  to?: LinkTarget;
  /** At the row's trailing edge, muted: a shortcut as a `Kbd`, or a count. Not on a submenu. */
  endContent?: Child;
  /** Makes the row a submenu of these actions, one level deep. */
  items?: readonly DropdownMenuItemData[];
  /** @default false */
  isDisabled?: boolean;
  /** `destructive` draws the icon in the danger role. Not on a submenu. @default 'default' */
  variant?: DropdownMenuItemVariant;
  'data-testid'?: string;
}

/** A row of a menu given as data: an action or a submenu, a divider, or actions under a heading. */
export type DropdownMenuEntry =
  | DropdownMenuItemData
  | { type: 'divider' }
  | { type: 'group'; label: string; items: readonly DropdownMenuItemData[] };

// `span-inline-end` spans from the trigger's start edge toward the inline end, so the menu's start
// edge lines up with the trigger's; logical, so it mirrors in RTL.
const areas = {
  bottom: { start: 'bottom span-inline-end', center: 'bottom', end: 'bottom span-inline-start' },
  top: { start: 'top span-inline-end', center: 'top', end: 'top span-inline-start' },
} as const satisfies Record<DropdownMenuPlacement, Record<DropdownMenuAlignment, string>>;

/** The items' height; set by whatever holds them: a DropdownMenu or a ContextMenu. */
export const MenuSize = createContext<ButtonSize | null>(null);
/**
 * A panel's icon column: whether its rows keep an icon's width at their start, and what its rows found as they
 * drew. A row with an icon marks `found`; the panel reads it once laid out. Shared with ContextMenu, not public.
 */
interface Column { isShown: boolean; found: { hasIcon: boolean } }
export const IconColumn = createContext<Column>({ isShown: false, found: { hasIcon: false } });
/**
 * A panel's column, from its rows: data rows are known at once; composed rows tell it as they draw, and it
 * shows the column one render later, on its first layout. Give `sync` to the panel's `onLayout`.
 */
export const useIconColumn = (items: readonly DropdownMenuEntry[] | undefined) => {
  const seen = useLocal('iconColumn', false);
  const found = { hasIcon: false };
  return {
    value: { isShown: hasIcons(items) || seen.value, found } satisfies Column,
    sync: () => { if (found.hasIcon !== seen.value) seen.set(found.hasIcon); },
  };
};
/** Whether any row of these entries, in a group too, has an icon: a submenu's own rows are its own panel. */
const hasIcons = (entries: readonly DropdownMenuEntry[] | undefined): boolean =>
  entries !== undefined && entries.some(entry => 'type' in entry ? entry.type === 'group' && hasIcons(entry.items) : entry.icon !== undefined);
/** A row's lead: its icon, which it tells the panel of, or the icon's space when the panel's rows have icons. */
function Lead({ icon, className }: { icon: IconRef | undefined; className: string }) {
  const column = useContext(IconColumn);
  if (icon !== undefined) {
    column.found.hasIcon = true;
    return <Glyph name={icon} className={className} />;
  }
  return column.isShown ? <view aria-hidden className={r.itemIcon.base} /> : null;
}
/** Which submenu has taken over the panel, where there is no room to open one beside its item. */
interface DrillState {
  open: string | null;
  /** Shows a submenu in the panel, by its id and its name; `null` goes back to the menu. */
  set: (submenu: DrilledTo | null) => void;
  isCompact: boolean;
  /** The menu's name, which the back row names as where it goes. */
  parent: string;
}
/** Shared with ContextMenu, not public: a const, so the catalogue does not list it. */
export const Drill = createContext<DrillState | null>(null);
interface DrilledTo { id: string; label: string }
/**
 * A menu's drill state: which submenu took the panel, and the menu's name meanwhile, the submenu's, so a screen
 * reader says where it is. Shared with ContextMenu, not public.
 */
export const useDrill = (label: string, isCompact: boolean) => {
  const drilled = useLocal<DrilledTo | null>('drilled', null);
  return {
    state: { open: drilled.value?.id ?? null, set: drilled.set, isCompact, parent: label } satisfies DrillState,
    name: drilled.value?.label ?? label,
    isDrilled: drilled.value !== null,
    reset: () => drilled.set(null),
  };
};
/** Whether a submenu has replaced this item in the panel. */
const isReplaced = (drill: DrillState | null) => drill !== null && drill.open !== null;

/** Set by a holder that does not close by itself, such as a sheet: an item calls it after its action. */
export const MenuClose = createContext<(() => void) | null>(null);
const RadioGroup = createContext<DropdownMenuRadioGroupProps | null>(null);

/**
 * A menu of actions that opens from its trigger. The menu has the View's menu keyboard: arrows,
 * Home, End and type-ahead, opened from the keyboard on its first item.
 */
export function DropdownMenu({
  children,
  menu,
  items,
  label,
  size = 'md',
  isOpen,
  onOpenChange,
  placement = 'bottom',
  alignment = 'start',
  menuClassName,
  'data-testid': testId, ...attributes
}: DropdownMenuProps) {
  const id = useId();
  const isCompact = useView().env.sizeClass === 'compact';
  const drill = useDrill(label, isCompact);
  const column = useIconColumn(items);
  const rows = menuRows('DropdownMenu', menu, items);
  return (
    <>
      <Trigger wiring={{ popoverTarget: id }} data-testid={testId} {...dataOf(attributes)}>{children}</Trigger>
      <MenuSize.Provider value={size}>
        <view
          id={id}
          popover
          role="menu"
          aria-label={drill.name}
          open={isOpen}
          // A menu that closes forgets which submenu it was showing.
          onToggle={next => { if (!next) drill.reset(); onOpenChange?.(next); }}
          // Escape or back inside a submenu that took the panel goes up one level; from the top it closes the menu.
          onCloseRequest={() => { if (!drill.isDrilled) return true; drill.reset(); return false; }}
          data-testid={testId && `${testId}-menu`}
          style={{ positionArea: areas[placement][alignment] }}
          className={cx(r.menu.base, r.menu.offset[placement], isCompact && r.menu.drill, menuClassName)}
        >
          <Drill.Provider value={drill.state}>
            <IconColumn.Provider value={column.value}>
              <view onLayout={column.sync} className={r.items.base}>{rows}</view>
            </IconColumn.Provider>
          </Drill.Provider>
        </view>
      </MenuSize.Provider>
    </>
  );
}

/** A holder's rows from its `menu` or its `items`; exactly one must be given. Shared with ContextMenu, not public: a const, so the catalogue does not list it. */
export const menuRows = (owner: string, menu: Child | undefined, items: readonly DropdownMenuEntry[] | undefined): Child => {
  if ((menu === undefined) === (items === undefined)) throw Error(`${owner}: give either menu or items`);
  if (items === undefined) return menu;
  const keys = keysOf(owner, items);
  return items.map((entry, index) => entryRow(owner, entry, keys[index]));
};

/** Each row's key: its `id`, or its place. A repeated id is told about and the repeat falls back to its place, so it never takes the first's focus. */
function keysOf(owner: string, entries: readonly DropdownMenuEntry[]): string[] {
  const seen = new Set<string>();
  return entries.map((entry, index) => {
    const id = 'type' in entry ? undefined : entry.id;
    if (id === undefined) return `#${index}`;
    if (seen.has(id)) {
      warnOnce(`${owner}: two rows have the id "${id}"; an id names one row`);
      return `#${index}`;
    }
    seen.add(id);
    return id;
  });
}

function rowsOf(owner: string, items: readonly DropdownMenuItemData[]): Child[] {
  const keys = keysOf(owner, items);
  return items.map((item, index) => itemRow(owner, item, keys[index]));
}

function entryRow(owner: string, entry: DropdownMenuEntry, key: string): Child {
  if ('type' in entry) {
    if (entry.type === 'divider') return <DropdownMenuDivider key={key} />;
    return <DropdownMenuGroup key={key} label={entry.label}>{rowsOf(owner, entry.items)}</DropdownMenuGroup>;
  }
  return itemRow(owner, entry, key);
}

function itemRow(owner: string, item: DropdownMenuItemData, key: string): Child {
  if (item.items !== undefined) {
    return (
      <DropdownMenuSubmenu key={key} label={item.label} icon={item.icon} isDisabled={item.isDisabled} data-testid={item['data-testid']}>
        {rowsOf(owner, item.items)}
      </DropdownMenuSubmenu>
    );
  }
  const shared = { label: item.label, description: item.description, icon: item.icon, endContent: item.endContent, isDisabled: item.isDisabled, variant: item.variant, 'data-testid': item['data-testid'] };
  if (item.to !== undefined) {
    if (item.onClick !== undefined) throw Error(`${owner}: "${item.label}" takes onClick or to, not both`);
    if (item.isRepeatable) throw Error(`${owner}: "${item.label}" goes to a page, which closes the menu; isRepeatable is for onClick`);
    return <DropdownMenuItem key={key} {...shared} to={item.to} />;
  }
  if (item.onClick === undefined) throw Error(`${owner}: "${item.label}" needs onClick, to or items`);
  return <DropdownMenuItem key={key} {...shared} onClick={item.onClick} isRepeatable={item.isRepeatable} />;
}

/** One action in a DropdownMenu, or with `to` a way to a page. */
export function DropdownMenuItem(props: DropdownMenuItemProps) {
  const { label, description, icon, endContent, isDisabled = false, variant = 'default', 'data-testid': testId } = props;
  const size = useContext(MenuSize);
  const close = useContext(MenuClose);
  const drill = useContext(Drill);
  const descriptionId = `${useId()}-description`;
  if (size === null) throw Error('DropdownMenuItem: place it inside a DropdownMenu');
  if (isReplaced(drill)) return null;
  const content = (
    <>
      <Lead icon={icon} className={cx(r.itemIcon.base, r.itemIcon.variant[variant])} />
      <view className={r.itemText.base}>
        <text className={cx(r.itemLabel.base, r.itemLabel.variant[variant])}>{label}</text>
        {description !== undefined && <text id={descriptionId} className={r.itemDescription.base}>{description}</text>}
      </view>
      {endContent !== undefined && <view aria-hidden className={r.itemEnd.base}>{endContent}</view>}
    </>
  );
  const shared = {
    role: 'menuitem',
    'aria-label': label,
    'aria-describedby': description !== undefined ? descriptionId : undefined,
    disabled: isDisabled,
    'data-testid': testId,
    ...dataOf(props),
    className: cx(r.item.base, r.item.size[size]),
  } as const;
  // A link row takes no press of its own: the engine closes a menu when one of its items is chosen.
  if (props.to !== undefined) {
    if (props.isRepeatable) throw Error(`DropdownMenuItem "${label}": a link closes the menu; isRepeatable is for onClick`);
    return <link {...shared} to={props.to}>{content}</link>;
  }
  const { onClick, isRepeatable = false } = props;
  if (isRepeatable) return <button {...shared} keepOpen onClick={onClick}>{content}</button>;
  return <button {...shared} onClick={() => { onClick(); close?.(); }}>{content}</button>;
}

/** An item that turns one option on or off, checked at its end. */
export function DropdownMenuCheckboxItem({ label, description, value: isChecked, isIndeterminate = false, onChange: onCheckedChange, isDisabled = false, 'data-testid': testId }: DropdownMenuCheckboxItemProps) {
  const isMixed = isIndeterminate && !isChecked;
  return (
    <CheckableItem role="menuitemcheckbox" label={label} description={description} isChecked={isChecked} isMixed={isMixed} isDisabled={isDisabled}
      onClick={() => onCheckedChange(isMixed || !isChecked)} testId={testId} owner="DropdownMenuCheckboxItem" />
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
  if (isReplaced(useContext(Drill))) return null;
  return (
    <view role="group" aria-label={label} className={r.group.base}>
      <text aria-hidden className={r.groupLabel.base}>{label}</text>
      {children}
    </view>
  );
}

/** DropdownMenuRadioItems of which one is checked. It draws nothing of its own; set it off with dividers. */
export function DropdownMenuRadioGroup(props: DropdownMenuRadioGroupProps) {
  if (isReplaced(useContext(Drill))) return null;
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
  isMixed?: boolean;
  isDisabled: boolean;
  onClick: () => void;
  testId: string | undefined;
  owner: string;
}) {
  const size = useContext(MenuSize);
  const drill = useContext(Drill);
  const descriptionId = `${useId()}-description`;
  if (size === null) throw Error(`${props.owner}: place it inside a DropdownMenu`);
  if (isReplaced(drill)) return null;
  return (
    <button
      role={props.role}
      aria-checked={props.isMixed ? 'mixed' : props.isChecked}
      aria-label={props.label}
      aria-describedby={props.description !== undefined ? descriptionId : undefined}
      disabled={props.isDisabled}
      onClick={props.onClick}
      data-testid={props.testId}
      className={cx(r.item.base, r.item.size[size])}
    >
      <Lead icon={undefined} className={r.itemIcon.base} />
      <view className={r.itemText.base}>
        <text className={r.itemLabel.base}>{props.label}</text>
        {props.description !== undefined && <text id={descriptionId} className={r.itemDescription.base}>{props.description}</text>}
      </view>
      {props.isChecked && <Glyph name="check" weight={2.5} className={r.itemCheck.base} />}
      {props.isMixed && <view aria-hidden className={r.itemDash.base} />}
    </button>
  );
}

export interface DropdownMenuSubmenuProps {
  /** The item's text, and the submenu's accessible name. */
  label: string;
  /** An icon by name from the set, or a definition, before the label; hidden from assistive tech. */
  icon?: IconRef;
  /** The submenu's items. One level only: a submenu holds no submenu. */
  children: Child;
  /** @default false */
  isDisabled?: boolean;
  /** On the item; the submenu gets `<testid>-menu`. */
  'data-testid'?: string;
}

/** An item that opens a second menu at its side, for a few choices that belong under one name: "Move to". */
export function DropdownMenuSubmenu({ label, icon, children, isDisabled = false, 'data-testid': testId, ...attributes }: DropdownMenuSubmenuProps) {
  const size = useContext(MenuSize);
  const drill = useContext(Drill);
  const id = useId();
  const column = useIconColumn(undefined);
  if (size === null) throw Error('DropdownMenuSubmenu: place it inside a DropdownMenu');
  const row = (
    <>
      <Lead icon={icon} className={cx(r.itemIcon.base, r.itemIcon.variant.default)} />
      <view className={r.itemText.base}>
        <text className={r.itemLabel.base}>{label}</text>
      </view>
      <Glyph name="chevron-end" className={r.submenuChevron.base} />
    </>
  );
  // No room beside the item: the submenu takes the panel, under a row back to the menu, as the reference does.
  if (drill?.isCompact) {
    if (drill.open === id) {
      // The menu's name sits in a sentence of the UI's language: isolated, so a name in the other direction keeps its order.
      const back = liuliText('@liuli.backTo', { parent: `\u2068${drill.parent}\u2069` });
      return (
        <>
          <button role="menuitem" keepOpen aria-label={back} aria-expanded onClick={() => drill.set(null)} data-testid={testId && `${testId}-back`} className={cx(r.item.base, r.item.size[size], r.back.base)}>
            <Glyph name="chevron-start" className={r.submenuChevron.base} />
            <text className={r.back.label}>{back}</text>
          </button>
          <view aria-hidden className={r.divider.base} />
          <Drill.Provider value={{ ...drill, open: null }}>
            <IconColumn.Provider value={column.value}>
              <view onLayout={column.sync} role="group" aria-label={label} data-testid={testId && `${testId}-menu`} className={r.group.base}>{children}</view>
            </IconColumn.Provider>
          </Drill.Provider>
        </>
      );
    }
    if (drill.open !== null) return null;
    // A menu item that keeps its menu open, so the arrow keys reach it like any other row.
    return (
      <button role="menuitem" keepOpen aria-label={label} aria-expanded={false} disabled={isDisabled} onClick={() => drill.set({ id, label })} data-testid={testId} {...dataOf(attributes)} className={cx(r.item.base, r.item.size[size])}>
        {row}
      </button>
    );
  }
  return (
    <>
      <button role="menuitem" popoverTarget={id} aria-label={label} disabled={isDisabled} data-testid={testId} {...dataOf(attributes)} className={cx(r.item.base, r.item.size[size])}>
        {row}
      </button>
      <IconColumn.Provider value={column.value}>
        <view
          onLayout={column.sync}
          id={id}
          popover
          role="menu"
          aria-label={label}
          data-testid={testId && `${testId}-menu`}
          // Beside the item, tops lined up; it flips to the other side, or drops below, for room.
          style={{ positionArea: 'inline-end span-block-end' }}
          className={cx(r.menu.base, r.menu.side)}
        >
          {children}
        </view>
      </IconColumn.Provider>
    </>
  );
}

/** A hairline between groups of items; hidden from assistive tech. */
export function DropdownMenuDivider() {
  if (isReplaced(useContext(Drill))) return null;
  return <view aria-hidden className={r.divider.base} />;
}
