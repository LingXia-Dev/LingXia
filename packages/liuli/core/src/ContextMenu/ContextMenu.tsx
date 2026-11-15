import { cx, useId, useLocal, useView, type Child } from '@lingxia/view';
import { MenuSize } from '../DropdownMenu/DropdownMenu';
import { contextMenuRecipe as r } from './ContextMenu.recipe';

export interface ContextMenuProps {
  /** Names the menu for assistive tech, and heads it when it docks: "Trip to the coast". */
  label: string;
  /** What the menu is for: a row, a card, a picture. A long press or a secondary click on it asks for the menu. */
  children: Child;
  /** `DropdownMenuItem`s, dividers and groups, in order. The same actions must be reachable another way. */
  menu: Child;
  /** @default false */
  isDisabled?: boolean;
  /** Classes for the wrapper around `children`. */
  className?: string;
  /** On the wrapper; the menu gets `<testid>-menu`. */
  'data-testid'?: string;
}

type Ask = { x: number; y: number; pointer: 'touch' | 'mouse' | 'keyboard' };

/** A menu for one thing, asked for on the thing itself: a long press, a secondary click, the Menu key. */
export function ContextMenu({ label, children, menu, isDisabled = false, className, 'data-testid': testId }: ContextMenuProps) {
  const id = useId();
  const ask = useLocal<Ask | null>('ask', null);
  const { env } = useView();
  const at = ask.value;
  const isDocked = at?.pointer === 'touch';
  // At a point: kept inside the page, and opening upward in the lower part of it.
  const place = at === null || isDocked
    ? { bottom: env.safeArea.bottom + 8 }
    : {
        left: Math.max(8, Math.min(at.x, env.viewport.width - 200)),
        ...(at.y > env.viewport.height * 0.6 ? { bottom: env.viewport.height - at.y } : { top: at.y }),
      };
  return (
    <>
      <view onContextMenu={isDisabled ? undefined : event => ask.set({ x: event.x, y: event.y, pointer: event.pointer })} data-testid={testId} className={cx(r.target.base, className)}>
        {children}
      </view>
      <MenuSize.Provider value={isDocked ? 'lg' : 'md'}>
        <view
          id={id}
          popover
          role="menu"
          aria-label={label}
          open={at !== null}
          onToggle={isOpen => { if (!isOpen) ask.set(null); }}
          data-testid={testId && `${testId}-menu`}
          style={place}
          className={cx(r.menu.base, isDocked ? r.menu.docked : r.menu.point)}
        >
          {isDocked && <text aria-hidden className={r.title.base}>{label}</text>}
          {menu}
        </view>
      </MenuSize.Provider>
    </>
  );
}
