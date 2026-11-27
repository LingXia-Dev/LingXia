import { cx } from '@lingxia/view';
import { extractOutline } from '../schema/helpers';
import type { JinshuDocument, OutlineItem } from '../schema/types';
import type { JinshuNavHandle } from '../internal/nav';
import { jinshuText } from '../i18n/text';
import { jinshuOutlineRecipe as r } from './JinshuOutline.recipe';

export { useJinshuNav, type JinshuNavHandle, type JinshuNavState } from '../internal/nav';
// What `items` takes and how Logic makes it, so the outline needs no second import.
export { extractOutline } from '../schema/helpers';
export type { OutlineItem } from '../schema/types';

export type OutlineSectionStatus = 'done' | 'current' | 'locked';

export interface JinshuOutlineProps {
  /** The document whose headings it lists. */
  readonly doc?: JinshuDocument;
  /** Headings taken out in Logic with `extractOutline`, in place of `doc`. */
  readonly items?: readonly OutlineItem[];
  /** Shared with the JinshuView or JinshuEditor it moves, from `useJinshuNav()`. */
  readonly nav?: JinshuNavHandle;
  /** Each section's progress by heading id: `done`, `current` or `locked`, shown at the row's end. */
  readonly status?: Readonly<Record<string, OutlineSectionStatus>>;
  /** The outline's heading and accessible name. Defaults to "Contents" in the page's language. */
  readonly title?: string;
  /** Hides the drawn title, for a sheet or popover that already shows one; it stays the accessible name. @default false */
  readonly isTitleHidden?: boolean;
  /** A heading was pressed; with `nav`, the reader also scrolls to it. */
  readonly onSelect?: (headingId: string) => void;
  readonly className?: string;
  readonly 'data-testid'?: string;
}

const STATUS_GLYPH: Record<OutlineSectionStatus, string> = { done: '✓', current: '●', locked: '○' };

/** Table of contents generated from document headings. */
export function JinshuOutline({
  doc,
  items,
  nav,
  status,
  title: ownTitle,
  isTitleHidden = false,
  onSelect,
  className,
  'data-testid': testId,
}: JinshuOutlineProps) {
  const title = ownTitle ?? jinshuText('@jinshu.outline.title');
  const list = items ?? (doc ? extractOutline(doc) : []);
  const activeId = nav?.state.activeHeadingId ?? null;

  if (list.length === 0) return null;

  return (
    <view role="group" aria-label={title} data-testid={testId} className={cx(r.root.base, className)}>
      {!isTitleHidden && <text className={r.title.base}>{title}</text>}
      {list.map(item => {
        const isActive = activeId === item.id;
        const st = status?.[item.id];
        const text = item.text || jinshuText('@jinshu.outline.untitled');
        return (
          <button
            key={item.id}
            aria-label={st ? jinshuText('@jinshu.outline.status', { heading: text, status: st }) : undefined}
            onClick={() => {
              nav?.scrollTo(item.id);
              onSelect?.(item.id);
            }}
            className={cx(r.item.base, r.item.level[item.level], isActive ? r.item.active : r.item.idle)}
          >
            <text className={cx(r.itemText.base, isActive ? r.itemText.active : item.level === 1 ? r.itemText.top : r.itemText.nested)}>
              {text}
            </text>
            {st && <text aria-hidden className={cx(r.status.base, r.status.tone[st])}>{STATUS_GLYPH[st]}</text>}
          </button>
        );
      })}
    </view>
  );
}
