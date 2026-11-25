import { cx, useId, useLocal, useView, type Child } from '@lingxia/view';
import { Drill, IconColumn, MenuSize, menuRows, useDrill, useIconColumn, type DropdownMenuEntry } from '../DropdownMenu/DropdownMenu';
import { contextMenuRecipe as r } from './ContextMenu.recipe';
import { dataOf } from '../internal/data';

export interface ContextMenuProps {
  /** Names the menu for assistive tech, and heads it when it docks: "Trip to the coast". */
  label: string;
  /** What the menu is for: a row, a card, a picture. A long press or a secondary click on it asks for the menu. */
  children: Child;
  /** `DropdownMenuItem`s, dividers and groups, in order. Give this or `items`, not both. The same actions must be reachable another way. */
  menu?: Child;
  /** The rows as data, as DropdownMenu's `items` takes them. */
  items?: readonly DropdownMenuEntry[];
  /** @default false */
  isDisabled?: boolean;
  /** Classes for the wrapper around `children`. */
  className?: string;
  /** On the wrapper; the menu gets `<testid>-menu`. */
  'data-testid'?: string;
}

type Ask = { x: number; y: number; pointer: 'touch' | 'mouse' | 'keyboard' };

/** A menu for one thing, asked for on the thing itself: a long press, a secondary click, the Menu key. */
export function ContextMenu({ label, children, menu, items, isDisabled = false, className, 'data-testid': testId, ...attributes }: ContextMenuProps) {
  const id = useId();
  const ask = useLocal<Ask | null>('ask', null);
  // The last ask outlives the menu: while it closes it keeps its form and its place.
  const shown = useLocal<Ask | null>('shown', null);
  const { env } = useView();
  const rows = menuRows('ContextMenu', menu, items);
  const at = ask.value ?? shown.value;
  const isDocked = at?.pointer === 'touch';
  const drill = useDrill(label, isDocked);
  const column = useIconColumn(items);
  // At a point: kept inside the page, and opening upward in the lower part of it.
  const isUpward = at !== null && at.y > env.viewport.height * 0.6;
  const place = at === null || isDocked
    ? undefined
    : {
        left: Math.max(8, Math.min(at.x, env.viewport.width - 200)),
        ...(isUpward ? { bottom: env.viewport.height - at.y } : { top: at.y }),
      };
  return (
    <>
      <view onContextMenu={isDisabled ? undefined : event => { const next = { x: event.x, y: event.y, pointer: event.pointer }; shown.set(next); ask.set(next); }} data-testid={testId} {...dataOf(attributes)} className={cx(r.target.base, className)}>
        {children}
      </view>
      <MenuSize.Provider value={isDocked ? 'lg' : 'md'}>
        {/* From a finger the sheet has no room beside a row: a submenu takes its place, as on a phone's DropdownMenu. */}
        <Drill.Provider value={drill.state}>
          <IconColumn.Provider value={column.value}>
            <view
              id={id}
              popover
              role="menu"
              aria-label={drill.name}
              onLayout={column.sync}
              open={ask.value !== null}
              // A menu that closes forgets which submenu it was showing; Escape or back inside one goes up a level.
              onToggle={isOpen => { if (!isOpen) { ask.set(null); drill.reset(); } }}
              onCloseRequest={() => { if (!drill.isDrilled) return true; drill.reset(); return false; }}
              data-testid={testId && `${testId}-menu`}
              style={place}
              className={cx(r.menu.base, isDocked ? r.menu.docked : cx(r.menu.point, r.menu.from[isUpward ? 'above' : 'below']))}
            >
              {isDocked && <text aria-hidden className={r.title.base}>{label}</text>}
              {rows}
            </view>
          </IconColumn.Provider>
        </Drill.Provider>
      </MenuSize.Provider>
    </>
  );
}
