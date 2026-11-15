import { useLocal } from '@lingxia/view';
import { Button } from '../Button/Button';
import { Carousel } from './Carousel';

const photo = (n: number) => `public/photos/photo-${n}.png`;
const photos = [
  [1, 'The bay at sunrise'],
  [2, 'Fields behind the house'],
  [3, 'Dusk over the ridge'],
  [4, 'Morning fog in the valley'],
] as const;

/** Photos of one place: the dots lie over the pictures. */
export function Photos() {
  return (
    <Carousel label="Photos of the house" dots="over" pageLabel={(page, count) => `Photo ${page + 1} of ${count}`} data-testid="photos">
      {photos.map(([n, alt]) => (
        <view key={alt} className="relative w-full aspect-4/3">
          <image src={photo(n)} alt={alt} className="absolute inset-0 size-full object-cover" />
        </view>
      ))}
    </Carousel>
  );
}

const steps = [
  ['Write anywhere', 'Notes open in a tap and save as you type.'],
  ['Find it again', 'Search reads every note, list and photo.'],
  ['Share what you choose', 'Each note is private until you invite someone.'],
] as const;

/** Pages of content, with the page held outside so a button can move it. */
export function Welcome() {
  const page = useLocal('step', 0);
  const isLast = Math.round(page.value) === steps.length - 1;
  return (
    <view className="gap-3">
      <Carousel label="Welcome" bind={page} pageLabel={(at, count) => `Step ${at + 1} of ${count}`} data-testid="welcome">
        {steps.map(([title, body]) => (
          <view key={title} className="gap-1 bg-surface-sunken p-5 min-h-28">
            <text className="text-callout font-semibold text-text">{title}</text>
            <text className="text-footnote text-muted">{body}</text>
          </view>
        ))}
      </Carousel>
      <Button label={isLast ? 'Get started' : 'Next'} variant="primary" width="fill" onClick={() => { if (!isLast) page.set(Math.round(page.value) + 1); }} data-testid="next" />
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <view className="w-32">
      <Carousel label="Sample" pageLabel={page => `Page ${page + 1}`}>
        <view className="h-12 bg-accent-soft" />
        <view className="h-12 bg-surface-sunken" />
        <view className="h-12 bg-accent-soft" />
      </Carousel>
    </view>
  );
}
