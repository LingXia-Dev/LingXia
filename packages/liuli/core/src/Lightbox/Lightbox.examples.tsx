import { useLocal } from '@lingxia/view';
import { Thumbnail } from '../Thumbnail/Thumbnail';
import { Lightbox, type LightboxProps } from './Lightbox';

const photo = (n: number) => `public/photos/photo-${n}.png`;
const photos = [
  { src: photo(1), alt: 'The bay at sunrise', caption: 'The bay at sunrise, from the terrace' },
  { src: photo(2), alt: 'Fields behind the house', caption: 'Fields behind the house' },
  { src: photo(3), alt: 'Dusk over the ridge', caption: 'Dusk over the ridge' },
  { src: photo(4), alt: 'Morning fog in the valley' },
] as const;

/** A grid of thumbnails: pressing one opens the pictures at that one. */
export function FromGrid() {
  const open = useLocal('open', false);
  const page = useLocal('page', 0);
  return (
    <view className="gap-2">
      <view className="grid grid-cols-4 gap-2">
        {photos.map((each, index) => (
          <Thumbnail key={each.alt} src={each.src} alt={each.alt} size="fill" onClick={() => { page.set(index); open.set(true); }} data-testid={`open-${index}`} />
        ))}
      </view>
      <Lightbox
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="Photos of the house"
        images={photos}
        bind={page}
        data-testid="viewer"
      />
    </view>
  );
}

/** One picture: no count and no dots. */
export function Single() {
  const open = useLocal('one', false);
  return (
    <view className="items-start">
      <Thumbnail src={photo(5)} alt="Dunes at noon" size="lg" ratio="landscape" onClick={() => open.set(true)} data-testid="one-open" />
      <Lightbox isOpen={open.value} onOpenChange={next => open.set(next)} label="Dunes at noon" images={[{ src: photo(5), alt: 'Dunes at noon' }]} pageLabel={() => ''} data-testid="one" />
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <view className="w-28 h-20 items-center justify-center rounded-control bg-media">
      <image src={photo(1)} alt="" className="w-20 aspect-4/3 object-cover" />
    </view>
  );
}

/** The playground's props that are not plain values. */
export function playground() {
  return {
    images: photos,
    pageLabel: (at: number, count: number) => `${at + 1} of ${count}`,
  } satisfies Partial<LightboxProps>;
}
