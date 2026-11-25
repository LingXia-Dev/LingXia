import { cx, useId, useView } from '@lingxia/view';
import { contextMenuRecipe as r } from '@liulidesign/core/ContextMenu/recipe';
import { MenuSize, menuRows, type DropdownMenuEntry } from '@liulidesign/core/DropdownMenu';

export type MenuAsk = { x: number; y: number; pointer: 'touch' | 'mouse' | 'keyboard' };

/**
 * ContextMenu's menu, shared by every row of a long list: each row only reports where the menu was
 * asked for, so a row carries a handler instead of a whole hidden menu.
 */
export function RowMenu({ label, at, items, onClose, 'data-testid': testId }: {
  label: string;
  at: MenuAsk | null;
  items: readonly DropdownMenuEntry[];
  onClose: () => void;
  'data-testid'?: string;
}) {
  const id = useId();
  const { env } = useView();
  const isDocked = at?.pointer === 'touch';
  const isUpward = at !== null && at.y > env.viewport.height * 0.6;
  const place = at === null || isDocked
    ? undefined
    : {
        left: Math.max(8, Math.min(at.x, env.viewport.width - 200)),
        ...(isUpward ? { bottom: env.viewport.height - at.y } : { top: at.y }),
      };
  return (
    <MenuSize.Provider value={isDocked ? 'lg' : 'md'}>
      <view
        id={id}
        popover
        role="menu"
        aria-label={label}
        open={at !== null}
        onToggle={(isOpen) => { if (!isOpen) onClose(); }}
        data-testid={testId}
        style={place}
        className={cx(r.menu.base, isDocked ? r.menu.docked : cx(r.menu.point, r.menu.from[isUpward ? 'above' : 'below']))}
      >
        {isDocked ? <text aria-hidden className={r.title.base}>{label}</text> : null}
        {at ? menuRows('RowMenu', undefined, items) : null}
      </view>
    </MenuSize.Provider>
  );
}
