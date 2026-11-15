import { Item, ItemList } from '../Item/Item';
import { Thumbnail } from './Thumbnail';

const photo = (n: number) => `public/photos/photo-${n}.png`;
const photos = [
  [1, 'The bay at sunrise'],
  [2, 'Fields behind the house'],
  [3, 'Dusk over the ridge'],
  [4, 'Morning fog in the valley'],
  [5, 'Dunes at noon'],
  [1, 'The bay, a minute later'],
] as const;

/** A grid of photos, each as wide as its column. */
export function Grid() {
  return (
    <view className="grid grid-cols-3 gap-2">
      {photos.map(([n, alt], index) => <Thumbnail key={alt} src={photo(n)} alt={alt} size="fill" onClick={() => {}} data-testid={`grid-${index}`} />)}
    </view>
  );
}

/** At the start of a row, for the thing the row is about. */
export function InRows() {
  return (
    <ItemList label="Albums">
      <Item label="Coast, November" description="48 photos" startContent={<Thumbnail src={photo(1)} alt="" size="sm" data-testid="row" />} hasChevron onClick={() => {}} />
      <Item label="The valley" description="112 photos" startContent={<Thumbnail src={photo(4)} alt="" size="sm" />} hasChevron onClick={() => {}} />
    </ItemList>
  );
}

export function Sizes() {
  return (
    <view className="flex-row flex-wrap items-end gap-3">
      <Thumbnail src={photo(3)} alt="Dusk over the ridge" size="sm" />
      <Thumbnail src={photo(3)} alt="Dusk over the ridge" />
      <Thumbnail src={photo(3)} alt="Dusk over the ridge" size="lg" />
      <Thumbnail src={photo(3)} alt="Dusk over the ridge" size="lg" ratio="landscape" />
    </view>
  );
}

/** A strip under a picture: the one in view is ringed. */
export function Strip() {
  return (
    <view className="flex-row gap-3 p-1">
      {photos.slice(0, 4).map(([n, alt], index) => <Thumbnail key={alt} src={photo(n)} alt={alt} size="sm" isSelected={index === 1} onClick={() => {}} data-testid={`strip-${index}`} />)}
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <view className="flex-row gap-2">
      <Thumbnail src={photo(1)} alt="" size="sm" />
      <Thumbnail src={photo(3)} alt="" size="sm" />
    </view>
  );
}
