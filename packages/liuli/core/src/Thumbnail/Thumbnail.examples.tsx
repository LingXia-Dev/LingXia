import { useLocal } from '@lingxia/view';
import { Held, Said } from '../example';
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
    <Said>
      {say => (
        <view className="grid grid-cols-3 gap-2">
          {photos.map(([n, alt], index) => <Thumbnail key={alt} src={photo(n)} alt={alt} size="fill" onClick={() => say(`Opened ${alt}`)} data-testid={`grid-${index}`} />)}
        </view>
      )}
    </Said>
  );
}

/** At the start of a row, for the thing the row is about. */
export function InRows() {
  return (
    <Said>
      {say => (
        <ItemList label="Albums">
          <Item label="Coast, November" description="48 photos" startContent={<Thumbnail src={photo(1)} alt="" size="sm" data-testid="row" />} hasChevron onClick={() => say('Opened Coast, November')} data-testid="album-coast" />
          <Item label="The valley" description="112 photos" startContent={<Thumbnail src={photo(4)} alt="" size="sm" />} hasChevron onClick={() => say('Opened The valley')} />
        </ItemList>
      )}
    </Said>
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
    <Held value={1}>
      {(shown, show) => (
        <view className="flex-row gap-3 p-1">
          {photos.slice(0, 4).map(([n, alt], index) => <Thumbnail key={alt} src={photo(n)} alt={alt} size="sm" isSelected={index === shown} onClick={() => show(index)} data-testid={`strip-${index}`} />)}
        </view>
      )}
    </Held>
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

/** Pictures attached to a message: each can be taken back, and one is still uploading. */
export function Attachments() {
  const attached = useLocal<readonly number[]>('attached', [0, 1, 2]);
  return (
    <view className="flex-row gap-3 pt-2 pe-2">
      {attached.value.map(index => {
        const [n, alt] = photos[index];
        return (
          <Thumbnail
            key={alt}
            src={photo(n)}
            alt={alt}
            size="md"
            isLoading={index === 2}
            onRemove={() => attached.set(attached.value.filter(each => each !== index))}
            data-testid={`attached-${index}`}
          />
        );
      })}
    </view>
  );
}
