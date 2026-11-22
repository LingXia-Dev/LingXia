import { useLocal } from '@lingxia/view';
import { unused } from '../example';
import { PageControl, type PageControlProps } from './PageControl';

const steps = [
  ['Write anywhere', 'Notes open in a tap and save as you type.'],
  ['Find it again', 'Search reads every note, list and photo.'],
  ['Share what you choose', 'Each note is private until you invite someone.'],
] as const;

/** Under what it pages: the dots follow the page, and pressing one goes there. */
export function Welcome() {
  const page = useLocal('page', 0);
  const [title, body] = steps[page.value];
  return (
    <view className="gap-3">
      <view className="gap-1 rounded-container bg-surface-sunken p-5">
        <text className="text-callout font-semibold text-text">{title}</text>
        <text className="text-footnote text-muted">{body}</text>
      </view>
      <PageControl label="Welcome steps" count={steps.length} bind={page} pageLabel={at => `Step ${at + 1} of ${steps.length}`} data-testid="welcome" />
    </view>
  );
}

/** Over a picture, in the colour that reads on media. */
export function OnMedia() {
  const page = useLocal('photo', 1);
  return (
    <view className="h-40 justify-end rounded-container bg-media pb-1">
      <PageControl label="Photos" count={5} bind={page} tone="on-media" pageLabel={at => `Photo ${at + 1} of 5`} data-testid="photos" />
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return <PageControl label="Pages" count={4} value={1} onChange={unused} />;
}

/** The playground's props that are not plain values. */
export function playground() {
  return { pageLabel: (at: number) => `Step ${at + 1} of 4` } satisfies Partial<PageControlProps>;
}
