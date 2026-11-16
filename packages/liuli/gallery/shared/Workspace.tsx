import { useLocal } from '@lingxia/view';
import { NavList, type NavListSection } from '@liulidesign/core/NavList';
import { SplitView } from '@liulidesign/core/SplitView';
import { Content as Overview } from '../content/home';
import { entries, galleryCategories, packageStatus, version } from './catalog.generated';
import { contents } from './contents.generated';

// The gallery in a wide window: every page in one sidebar, the chosen one beside it. It shows the
// same content the phone pages do, so a page reads the same in both.

const overview = 'home';
const sections: NavListSection[] = [
  { id: 'start', items: [{ value: overview, label: 'Overview' }] },
  ...galleryCategories.map(category => ({
    id: category.id,
    title: category.title,
    items: entries.filter(entry => entry.category === category.id).map(entry => ({ value: entry.page, label: entry.name })),
  })),
];
const titles = new Map<string, string>([
  ...entries.map(entry => [entry.page, entry.name] as [string, string]),
  ...galleryCategories.map(category => [category.page, category.title] as [string, string]),
]);

export interface WorkspaceProps {
  /** The page to show until one is chosen: Logic's record of the last choice, or a link's `page`. */
  page: string;
  /** Tells Logic what was chosen. */
  onSelect: (page: string) => void;
}

export function Workspace({ page, onSelect }: WorkspaceProps) {
  const chosen = useLocal<string | null>('chosen', null);
  const requested = chosen.value ?? page;
  const current = titles.has(requested) ? requested : overview;
  const select = (next: string) => {
    chosen.set(next);
    onSelect(next);
  };
  const Page = contents[current];
  const title = titles.get(current) ?? 'Overview';
  return (
    <page className="bg-canvas">
      {/* Test probe: the workspace is the regular form; see GalleryPage. */}
      <view aria-hidden data-testid="form-regular" className="absolute start-0 top-0 size-px" />
      <SplitView
        sidebarLabel="Gallery"
        contentLabel={title}
        contentKey={current}
        contentWidth="prose"
        sidebar={
          <>
            <view className="gap-0.5 px-3 pt-1">
              <view className="flex-row items-baseline gap-2">
                <text className="text-title font-semibold tracking-tight text-text">Liuli</text>
                <text className="text-body text-muted">琉璃</text>
              </view>
              <text className="text-footnote text-faint">{`${version} · ${packageStatus}`}</text>
            </view>
            <NavList label="Gallery pages" sections={sections} value={current} onChange={select} data-testid="gallery-nav" />
          </>
        }
        data-testid="gallery"
      >
        <text role="heading" headingLevel={1} className="-mb-4 text-title font-semibold tracking-tight text-text" data-testid="gallery-title">{title}</text>
        {Page === undefined ? <Overview onOpen={select} hasMasthead={false} /> : <Page onOpen={select} />}
      </SplitView>
    </page>
  );
}
