import { cx, reveal, useId, useLocal, useView } from '@lingxia/view';
import { liuliText } from '../i18n/text';
import type { IconRef } from '../Icon/Icon';
import { Glyph } from '../internal/Glyph';
import { SpinnerMark } from '../internal/spinner';
import { Kbd } from '../Kbd/Kbd';
import { commandPaletteRecipe as r } from './CommandPalette.recipe';
import { dataOf } from '../internal/data';

export interface CommandItem {
  /** What `onSelect` carries, and `<testid>-<id>` on its row. Unique in the palette. */
  id: string;
  label: string;
  /** A second line: where it leads, what it changes. */
  description?: string;
  icon?: IconRef;
  /** Other words it is found by, not shown: "remove" and "trash" for "Delete". A match here ranks after every match on a label. */
  keywords?: readonly string[];
  /** Quiet text at the end of the row: a kind, a count. Read after the label. */
  endText?: string;
  /** Its keyboard shortcut, drawn as key caps at the end of the row and read after the label; left out on a phone. */
  shortcut?: { keys: readonly string[]; label?: string };
  /** @default false */
  isDisabled?: boolean;
}

export interface CommandGroup {
  /** The group\'s heading and accessible name: "Go to", "Actions". */
  label: string;
  items: readonly CommandItem[];
}

export interface CommandPaletteProps {
  /** Whether it is shown. A closed palette renders nothing, so it opens empty each time. */
  isOpen: boolean;
  /** Called with `false` on a close request and after a choice. */
  onOpenChange: (isOpen: boolean) => void;
  /** The palette\'s accessible name, and the search line\'s: "Search commands". */
  label: string;
  placeholder?: string;
  /** Everything that can be chosen, in groups. Liuli filters them by what is typed, unless the app does. */
  groups: readonly CommandGroup[];
  /** Runs with the chosen item\'s id; the palette then closes. */
  onSelect: (id: string) => void;
  /** Hands filtering to the app: called with the text as it is typed, and `groups` is shown as given. For results fetched from Logic. */
  onQueryChange?: (query: string) => void;
  /** Shows a spinner in the search line while the app fetches. @default false */
  isLoading?: boolean;
  /** Shown when nothing matches: "No commands match". Defaults to "No matches" in the page's language. */
  emptyLabel?: string;
  'data-testid'?: string;
}

const inWord = /[\p{L}\p{N}]/u;

/** How well a label matches: 0 from its start, 1 from the start of a later word, 2 inside a word, -1 not at all. */
function rank(label: string, typed: string): number {
  const text = label.toLowerCase();
  let at = text.indexOf(typed);
  if (at <= 0) return at;
  let best = 2;
  for (; at >= 0 && best === 2; at = text.indexOf(typed, at + 1)) if (!inWord.test(text[at - 1])) best = 1;
  return best;
}

/** The label's rank, else 3 when a keyword matches from the start of one of its words. */
function itemRank(item: CommandItem, typed: string): number {
  const own = rank(item.label, typed);
  if (own >= 0) return own;
  return item.keywords?.some(word => { const at = rank(word, typed); return at === 0 || at === 1; }) ? 3 : -1;
}

function filtered(groups: readonly CommandGroup[], typed: string): readonly CommandGroup[] {
  if (typed === '') return groups;
  return groups
    .map(group => {
      const tiers: CommandItem[][] = [[], [], [], []];
      for (const item of group.items) tiers[itemRank(item, typed)]?.push(item);
      return { label: group.label, items: tiers.flat() };
    })
    .filter(group => group.items.length > 0);
}

/** Type to find a command or a place, and run it from the keyboard. */
export function CommandPalette({ isOpen, onOpenChange, label, placeholder, groups, onSelect, onQueryChange, isLoading = false, emptyLabel = liuliText('@liuli.noMatches'), 'data-testid': testId, ...attributes }: CommandPaletteProps) {
  const listId = useId();
  // Named by whether it is open, so a palette closed from outside opens again empty, at the first row.
  const session = isOpen ? 'open' : 'closed';
  const query = useLocal(`query-${session}`, '');
  const active = useLocal(`active-${session}`, -1);
  const { env } = useView();
  const isCompact = env.sizeClass === 'compact';
  const shown = onQueryChange ? groups : filtered(groups, query.value.trim().toLowerCase());
  const flat = shown.flatMap(group => group.items);
  const rowId = (index: number) => `${listId}-${index}`;
  const close = () => { onOpenChange(false); query.set(''); active.set(-1); };
  const choose = (item: CommandItem | undefined) => { if (item && !item.isDisabled) { onSelect(item.id); close(); } };
  // The next row that can be chosen, from `from` in `direction`, stopping at the ends; -1 when none is.
  const step = (from: number, direction: 1 | -1) => {
    for (let index = from + direction; index >= 0 && index < flat.length; index += direction) if (!flat[index].isDisabled) return index;
    return from >= 0 && from < flat.length ? from : -1;
  };
  // The highlighted row is brought into view as it moves.
  const move = (index: number) => { if (index < 0) return; active.set(index); reveal(rowId(index)); };
  // The row moved to, while it can still be chosen; else the first that can be.
  const at = active.value >= 0 && active.value < flat.length && !flat[active.value].isDisabled ? active.value : step(-1, 1);
  const keys = {
    ArrowDown: () => move(step(at, 1)),
    ArrowUp: () => move(step(at, -1)),
    Home: () => move(step(-1, 1)),
    End: () => move(step(flat.length, -1)),
    Enter: () => choose(flat[at]),
  };
  let index = -1;
  return (
    <dialog
      open={isOpen}
      onClose={close}
      closedby="any"
      aria-label={label}
      data-testid={testId} {...dataOf(attributes)}
      // On a wide screen it sits an eighth of the way down; on a phone it is docked to the top edge.
      style={isCompact ? undefined : { marginTop: Math.round(env.viewport.height * 0.12) }}
      className={cx(r.panel.base, isCompact ? r.panel.compact : r.panel.regular)}
    >
      <view className={r.search.base}>
        <Glyph name="search" side={20} className={r.searchIcon.base} />
        <input
          role="combobox"
          aria-label={label}
          aria-expanded
          aria-controls={listId}
          aria-activedescendant={at >= 0 ? rowId(at) : undefined}
          value={query.value}
          placeholder={placeholder}
          autoFocus
          enterKeyHint="go"
          onInput={text => { query.set(text); active.set(-1); onQueryChange?.(text); }}
          onKeyDown={keys}
          onSubmit={() => choose(flat[at])}
          data-testid={testId && `${testId}-search`}
          className={r.input.base}
        />
        {isLoading && <SpinnerMark className={r.spinner.base} />}
      </view>
      {flat.length === 0 ? (
        <text role="status" className={r.note.base}>{emptyLabel}</text>
      ) : (
        <scroll className={r.list.base}>
          <view id={listId} role="listbox" aria-label={label} className={r.group.base}>
            {shown.map(group => (
              <view key={group.label} role="group" aria-label={group.label} className={r.group.base}>
                <text aria-hidden className={r.groupLabel.base}>{group.label}</text>
                {group.items.map(item => {
                  index += 1;
                  const mine = index;
                  const shortcut = isCompact ? undefined : item.shortcut;
                  const spoken = [item.endText, shortcut && (shortcut.label ?? shortcut.keys.join(' '))].filter(part => part !== undefined);
                  return (
                    <button
                      key={item.id}
                      id={rowId(mine)}
                      role="option"
                      aria-selected={mine === at}
                      aria-label={spoken.length === 0 ? undefined : [item.label, ...spoken].join(', ')}
                      disabled={item.isDisabled}
                      onClick={() => choose(item)}
                      data-testid={testId && `${testId}-${item.id}`}
                      className={cx(r.option.base, mine === at && r.option.active)}
                    >
                      {item.icon !== undefined && <Glyph name={item.icon} className={r.optionIcon.base} />}
                      <view className={r.optionText.base}>
                        <text className={r.optionLabel.base}>{item.label}</text>
                        {item.description !== undefined && <text className={r.optionDescription.base}>{item.description}</text>}
                      </view>
                      {item.endText !== undefined && <text aria-hidden className={r.optionEnd.base}>{item.endText}</text>}
                      {shortcut !== undefined && <Kbd keys={shortcut.keys} className={r.optionShortcut.base} />}
                    </button>
                  );
                })}
              </view>
            ))}
          </view>
        </scroll>
      )}
    </dialog>
  );
}
