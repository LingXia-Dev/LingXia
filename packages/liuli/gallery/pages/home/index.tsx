import { useLocal, type Child, type View } from '@lingxia/view';
import { EmptyState } from '@liulidesign/core/EmptyState';
import { Item, ItemList } from '@liulidesign/core/Item';
import { TextInput } from '@liulidesign/core/TextInput';
import { entries, galleryCategories, newest, packageStatus, version, type GalleryEntry } from '../../shared/catalog.generated';
import { Category, GalleryPage, Masthead, PageSection, Tile } from '../../shared/Layout';
import * as previews from '../../shared/Previews';

// An index, not a wall of tiles: search, one card per category, then what is new. Everything it
// lists comes from shared/catalog.generated.ts, so a new component needs no edit here.

type Actions = { open(target: { page: string }): Promise<void> };

const titles = new Map(galleryCategories.map(category => [category.id, category.title]));
const pascal = (id: string) => id.replace(/(^|-)([a-z])/g, (_, __, letter: string) => letter.toUpperCase());
const preview = (id: string): Child => {
  const Preview = (previews as Record<string, (() => Child) | undefined>)[`${pascal(id)}Preview`];
  return Preview ? Preview() : null;
};
const countLine = (id: string, count: number) =>
  id === 'foundations' ? `${count} guides` : id === 'patterns' ? `${count} patterns` : `${count} ${count === 1 ? 'component' : 'components'}`;

/** Whole words of the query, each found in the name, the summary or a keyword. */
function matches(query: string): GalleryEntry[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  return entries.filter(entry => {
    const haystack = [entry.name, entry.summary, ...entry.keywords, titles.get(entry.category) ?? ''].join(' ').toLowerCase();
    return words.every(word => haystack.includes(word));
  });
}

export function render({ actions }: View<Actions>) {
  const query = useLocal('query', '');
  const found = query.value.trim() ? matches(query.value) : null;
  const open = (page: string) => () => actions.open({ page });
  return (
    <GalleryPage>
      <view className="gap-4">
        <Masthead version={version} status={packageStatus} />
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
      {found === null ? (
        <>
          <Category title="Browse">
            {galleryCategories.map((category, index) => (
              <Tile
                key={category.id}
                name={category.title}
                tagline={countLine(category.id, category.count)}
                isWide={galleryCategories.length % 2 === 1 && index === galleryCategories.length - 1}
                onOpen={open(category.page)}
                testId={`open-${category.page}`}
              >
                {preview(category.id)}
              </Tile>
            ))}
          </Category>
          <PageSection title="New">
            <ItemList label="New components">
              {newest.map(name => entries.find(entry => entry.name === name)!).map(entry => (
                <Item key={entry.name} label={entry.name} description={entry.summary} hasChevron onClick={open(entry.page)} data-testid={`new-${entry.page}`} />
              ))}
            </ItemList>
          </PageSection>
        </>
      ) : found.length ? (
        <PageSection title={found.length === 1 ? '1 result' : `${found.length} results`}>
          <ItemList label="Results">
            {found.map(entry => (
              <Item
                key={entry.page}
                label={entry.name}
                description={entry.summary}
                endContent={<text className="text-footnote text-muted">{titles.get(entry.category) ?? ''}</text>}
                onClick={open(entry.page)}
                data-testid={`result-${entry.page}`}
              />
            ))}
          </ItemList>
        </PageSection>
      ) : (
        <EmptyState title="Nothing matches" description={`No component, guide or pattern matches “${query.value.trim()}”.`} data-testid="home-no-results" />
      )}
    </GalleryPage>
  );
}
