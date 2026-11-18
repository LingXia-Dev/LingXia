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
  // The last ask outlives the menu: while it closes it keeps its form and its place.
  const shown = useLocal<Ask | null>('shown', null);
  const { env } = useView();
  const at = ask.value ?? shown.value;
  const isDocked = at?.pointer === 'touch';
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
      <view onContextMenu={isDisabled ? undefined : event => { const next = { x: event.x, y: event.y, pointer: event.pointer }; shown.set(next); ask.set(next); }} data-testid={testId} className={cx(r.target.base, className)}>
        {children}
      </view>
      <MenuSize.Provider value={isDocked ? 'lg' : 'md'}>
        <view
          id={id}
          popover
          role="menu"
          aria-label={label}
          open={ask.value !== null}
          onToggle={isOpen => { if (!isOpen) ask.set(null); }}
          data-testid={isDocked ? undefined : testId && `${testId}-menu`}
          style={place}
          className={cx(r.menu.base, isDocked ? r.menu.docked : cx(r.menu.point, r.menu.from[isUpward ? 'above' : 'below']))}
        >
          {/* Hidden from assistive tech, which closes the menu with Escape or back; the name only satisfies the button. */}
          {isDocked && <button aria-hidden aria-label={label} onClick={() => ask.set(null)} data-testid={testId && `${testId}-scrim`} className={r.scrim.base} />}
          {isDocked
            ? (
              <view data-testid={testId && `${testId}-menu`} className={r.sheet.base} style={{ marginBottom: env.safeArea.bottom + 8 }}>
                <text aria-hidden className={r.title.base}>{label}</text>
                {menu}
              </view>
            )
            : menu}
        </view>
      </MenuSize.Provider>
    </>
  );
}
