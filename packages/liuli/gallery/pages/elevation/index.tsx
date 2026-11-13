import { GalleryPage, PageSection } from '../../shared/Layout';

// Each level drawn as Liuli's components use it: a raised surface keeps its hairline in both
// themes, since a shadow alone barely shows on a dark page.

const levels = [
  { name: 'Flat', use: 'Cards and lists: a hairline, no shadow', className: 'rounded-container border border-text/12 bg-surface' },
  { name: 'Resting', use: 'A raised card: a contact shadow, one surface step up in dark', className: 'rounded-container border border-text/12 bg-surface-raised shadow-contact' },
  { name: 'Raised', use: 'Menus and anything that floats over the page', className: 'rounded-container border border-text/12 bg-surface-raised shadow-raised' },
];

export function render() {
  return (
    <GalleryPage>
      <text className="text-body text-muted">
        Most of the page stays flat; cards rest on it with a contact shadow, and only overlays (menus, popovers, dialogs, sheets) float with the raised shadow. A shadow says a thing sits above the page and will go away; each one is a tight contact shadow under a wide soft one, over a hairline that keeps the edge in dark.
      </text>
      <PageSection title="The two levels">
        <view className="flex-row gap-4 py-2">
          <view className="grow basis-0 gap-2">
            <view className="h-28 items-center justify-center rounded-container border border-text/12 bg-surface-raised shadow-raised">
              <text className="text-footnote font-medium text-muted">Raised</text>
            </view>
            <text className="text-footnote text-muted">A menu over the page</text>
          </view>
          <view className="grow basis-0 gap-2">
            <view className="h-28 justify-center rounded-container bg-scrim px-3">
              <view className="h-20 items-center justify-center rounded-dialog border border-text/12 bg-surface-raised shadow-modal">
                <text className="text-footnote font-medium text-muted">Modal</text>
              </view>
            </view>
            <text className="text-footnote text-muted">A dialog over the scrim</text>
          </view>
        </view>
      </PageSection>
      <PageSection title="When to use each">
        <view className="gap-6 py-2">
          {levels.map(level => (
            <view key={level.name} className={`h-24 justify-end gap-0.5 p-4 ${level.className}`}>
              <text className="text-body font-medium text-text">{level.name}</text>
              <text className="text-footnote text-muted">{level.use}</text>
            </view>
          ))}
          <view className="rounded-container bg-scrim p-4">
            <view className="h-24 justify-end gap-0.5 p-4 rounded-dialog border border-text/12 bg-surface-raised shadow-modal">
              <text className="text-body font-medium text-text">Modal</text>
              <text className="text-footnote text-muted">Dialogs, over a scrim that dims the page</text>
            </view>
          </view>
        </view>
      </PageSection>
    </GalleryPage>
  );
}
