import { cx, reveal, useId, useLocal } from '@lingxia/view';
import { Glyph } from '../internal/Glyph';
import { navListRecipe as r } from './NavList.recipe';

export interface NavListItem {
  /** What `value` and `onChange` carry: a page, a mailbox, a project. */
  value: string;
  /** The row's text and accessible name. */
  label: string;
  /** A short count or value at the end, such as unread messages; read after the label. */
  endText?: string;
  /** @default false */
  isDisabled?: boolean;
}

export interface NavListSection {
  /** Keys the section's open or closed state. */
  id: string;
  /** A heading that opens and closes the section; a section without one is always open. */
  title?: string;
  items: readonly NavListItem[];
}

export interface NavListProps {
  /** Names the list for assistive tech: "Mailboxes", "Components". */
  label: string;
  /** The rows, in sections. */
  sections: readonly NavListSection[];
  /** The current row's value, or `null` when none is current. */
  value: string | null;
  /** Runs with the value of a row chosen by a tap, a click or Enter. */
  onChange: (value: string) => void;
  /** Sections closed at first, by id; after that each remembers its own state. @default [] */
  defaultClosed?: readonly string[];
  className?: string;
  /** On the list; each row gets `<testid>-<value>`, each section heading `<testid>-section-<id>`. */
  'data-testid'?: string;
}

/**
 * A sidebar's list of places, read as a tree: one Tab stop, the arrows move between rows and Home and
 * End go to the ends; a section heading opens and closes its rows, and the current row sits on an
 * accent wash.
 */
export function NavList({ label, sections, value, onChange, defaultClosed = [], className, 'data-testid': testId }: NavListProps) {
  const closed = useLocal<readonly string[]>('closed', defaultClosed);
  const baseId = useId();
  const rowId = (row: string) => `${baseId}-${row}`;
  const isShown = value !== null && sections.some(section => (section.title === undefined || !closed.value.includes(section.id)) && section.items.some(item => item.value === value));
  const toggle = (id: string) => closed.set(closed.value.includes(id) ? closed.value.filter(each => each !== id) : [...closed.value, id]);
  return (
    <view role="tree" aria-label={label} data-testid={testId} className={cx(r.root.base, className)}>
      {sections.flatMap(section => {
        const isOpen = section.title === undefined || !closed.value.includes(section.id);
        const level = section.title === undefined ? 1 : 2;
        const heading = section.title === undefined ? [] : [
          <button
            key={`section-${section.id}`}
            role="treeitem"
            aria-level={1}
            aria-expanded={isOpen}
            onClick={() => toggle(section.id)}
            data-testid={testId && `${testId}-section-${section.id}`}
            className={r.group.base}
          >
            <text className={r.groupLabel.base}>{section.title}</text>
            <Glyph name="chevron-down" className={cx(r.groupChevron.base, !isOpen && r.groupChevron.closed)} />
          </button>,
        ];
        const rows = !isOpen ? [] : section.items.map(item => {
          const isSelected = item.value === value;
          return (
            <button
              key={item.value}
              role="treeitem"
              id={rowId(item.value)}
              aria-level={level}
              aria-selected={isSelected}
              aria-label={item.endText === undefined ? undefined : `${item.label}, ${item.endText}`}
              disabled={item.isDisabled}
              onClick={() => onChange(item.value)}
              data-testid={testId && `${testId}-${item.value}`}
              className={cx(r.item.base, isSelected ? r.item.selected : r.item.idle)}
            >
              <text className={cx(r.label.base, isSelected && r.label.selected)}>{item.label}</text>
              {item.endText !== undefined && <text aria-hidden className={r.end.base}>{item.endText}</text>}
            </button>
          );
        });
        return [...heading, ...rows];
      })}
      {/* A new mark for each current row: its first layout is the moment to bring that row into view,
          at launch and whenever the row is chosen from outside the list. */}
      {isShown && <view key={`current-${value}`} aria-hidden onLayout={() => reveal(rowId(value))} data-testid={testId && `${testId}-reveal`} className={r.current.base} />}
    </view>
  );
}
