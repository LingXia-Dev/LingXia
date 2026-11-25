import { cx, type LinkTarget } from '@lingxia/view';
import { DropdownMenu, DropdownMenuItem } from '../DropdownMenu/DropdownMenu';
import { IconButton } from '../IconButton/IconButton';
import { Glyph } from '../internal/Glyph';
import { breadcrumbRecipe as r } from './Breadcrumb.recipe';
import { dataOf } from '../internal/data';

interface BreadcrumbLevel {
  /** Identifies the level: its test id is `<id>-<key>`. */
  key: string;
  /** The level's name. */
  label: string;
}

/** A level: opened by `onClick`, or a link to its page with `to`, never both. The last item is where the person is, and takes neither. */
export type BreadcrumbItem = BreadcrumbLevel & (
  | { /** Opens the level. */ onClick?: () => void; to?: never }
  | { /** The level's page: `{ page, params }`, `{ tab }`, `{ back: 1 }`. Read as a link. */ to: LinkTarget; onClick?: never }
);

export interface BreadcrumbProps {
  /** Names the trail for assistive tech: "Breadcrumb", "You are in". */
  label: string;
  /** The levels from the top down; the last is the current page. */
  items: readonly BreadcrumbItem[];
  /** Past this many levels the middle ones fold into a menu; it needs `moreLabel`. @default 4 */
  maxItems?: number;
  /** Names the menu that holds the folded levels: "More levels". Without it nothing is folded. */
  moreLabel?: string;
  className?: string;
  'data-testid'?: string;
}


/** Where a page sits: the levels above it, each a way back up. */
export function Breadcrumb({ label, items, maxItems = 4, moreLabel, className, 'data-testid': testId, ...attributes }: BreadcrumbProps) {
  // Folded, the first level and the last two stay; the rest go into the menu, in order.
  const isFolded = moreLabel !== undefined && items.length > Math.max(3, maxItems);
  const folded = isFolded ? items.slice(1, -2) : [];
  const shown = isFolded ? [items[0], ...items.slice(-2)] : items;
  const level = (item: BreadcrumbItem, isCurrent: boolean) => {
    const id = testId && `${testId}-${item.key}`;
    const name = <text className={cx(r.label.base, r.label.link)}>{item.label}</text>;
    if (!isCurrent && item.to !== undefined) return <link to={item.to} data-testid={id} className={r.link.base}>{name}</link>;
    if (!isCurrent && item.onClick !== undefined) return <button onClick={item.onClick} data-testid={id} className={r.link.base}>{name}</button>;
    return <text data-testid={id} className={cx(r.label.base, isCurrent ? r.label.current : r.label.plain)}>{item.label}</text>;
  };
  const separator = <Glyph name="chevron-end" className={r.separator.base} />;
  return (
    <view role="list" aria-label={label} data-testid={testId} {...dataOf(attributes)} className={cx(r.root.base, className)}>
      {shown.map((item, index) => (
        <view key={item.key} role="listitem" className={r.item.base}>
          {index > 0 && separator}
          {isFolded && index === 1 && (
            <>
              <DropdownMenu
                label={moreLabel}
                size="sm"
                menu={folded.map(each => each.to !== undefined
                  ? <DropdownMenuItem key={each.key} label={each.label} to={each.to} data-testid={testId && `${testId}-${each.key}`} />
                  : <DropdownMenuItem key={each.key} label={each.label} onClick={each.onClick ?? (() => {})} isDisabled={!each.onClick} data-testid={testId && `${testId}-${each.key}`} />)}
              >
                <IconButton label={moreLabel} icon="more" size="sm" className={r.more.base} data-testid={testId && `${testId}-more`} />
              </DropdownMenu>
              {separator}
            </>
          )}
          {level(item, index === shown.length - 1)}
        </view>
      ))}
    </view>
  );
}
