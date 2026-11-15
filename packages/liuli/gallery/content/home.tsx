import { useLocal } from '@lingxia/view';
import { EmptyState } from '@liulidesign/core/EmptyState';
import { Item, ItemList } from '@liulidesign/core/Item';
import { TextInput } from '@liulidesign/core/TextInput';
import { entries, galleryCategories, packageStatus, version, type GalleryCategory, type GalleryEntry } from '../shared/catalog.generated';
import { Masthead } from '../shared/Layout';

// A table of contents: search, then the system in three groups, one row a category. Everything it
// lists comes from shared/catalog.generated.ts, so a new component needs no edit here.

const byId = new Map(galleryCategories.map(category => [category.id, category]));
const guides = new Set(['foundations', 'patterns']);
const groups: { title: string; rows: GalleryCategory[] }[] = [
  { title: 'Foundations', rows: galleryCategories.filter(category => category.id === 'foundations') },
  { title: 'Components', rows: galleryCategories.filter(category => !guides.has(category.id)) },
  { title: 'Patterns', rows: galleryCategories.filter(category => category.id === 'patterns') },
].filter(group => group.rows.length > 0);
// A group of one is named by its heading, so its row says what the group holds.
const rowLabel: Record<string, string> = { foundations: 'Guides', patterns: 'Screens' };

/** Whole words of the query, each found in the name, the summary or a keyword. */
function matches(query: string): GalleryEntry[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  return entries.filter(entry => {
    const haystack = [entry.name, entry.summary, ...entry.keywords, byId.get(entry.category)?.title ?? ''].join(' ').toLowerCase();
    return words.every(word => haystack.includes(word));
  });
}

/**
 * The index: search and the contents. `onOpen` takes a page name. The workspace names Liuli in
 * its sidebar, so it leaves out the masthead.
 */
export function Content({ onOpen, hasMasthead = true }: { onOpen: (page: string) => void; hasMasthead?: boolean }) {
  const query = useLocal('query', '');
  const found = query.value.trim() ? matches(query.value) : null;
  const open = (page: string) => () => onOpen(page);
  return (
    // A column a line of text can be read across, however wide the window.
    <view className="w-full max-w-[640px] gap-6">
      <view className="gap-4">
        {hasMasthead && <Masthead version={version} status={packageStatus} />}
        <TextInput
          value={query.value}
          onChange={next => query.set(next)}
          label="Search components"
          isLabelHidden
          placeholder="Search components"
          type="search"
          hasClear
          clearLabel="Clear search"
          data-testid="home-search"
        />
      </view>
      {found === null ? groups.map(group => (
        <ItemList key={group.title} title={group.title}>
          {group.rows.map(category => (
            <Item
              key={category.id}
              label={rowLabel[category.id] ?? category.title}
              description={category.description}
              startIcon={category.icon}
              endContent={<text className="text-footnote text-muted tabular-nums">{String(category.count)}</text>}
              hasChevron
              onClick={open(category.page)}
              data-testid={`open-${category.page}`}
            />
          ))}
        </ItemList>
      )) : found.length ? (
        <ItemList title={found.length === 1 ? '1 result' : `${found.length} results`}>
          {found.map(entry => (
            <Item
              key={entry.page}
              label={entry.name}
              description={entry.summary}
              startIcon={byId.get(entry.category)?.icon}
              hasChevron
              onClick={open(entry.page)}
              data-testid={`result-${entry.page}`}
            />
          ))}
        </ItemList>
      ) : (
        <EmptyState title="Nothing matches" description={`No component, guide or pattern matches “${query.value.trim()}”.`} data-testid="home-no-results" />
      )}
    </view>
  );
}
