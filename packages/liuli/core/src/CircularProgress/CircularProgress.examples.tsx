import { Item, ItemList } from '../Item/Item';
import { CircularProgress, CircularProgressGroup } from './CircularProgress';

/** A goal on a tile: the ring, and the app's words in its middle. */
export function Goal() {
  return (
    <view className="flex-row items-center gap-4">
      <CircularProgress label="Steps today" value={7200} max={10000} valueLabel="72%" size="lg" data-testid="steps" />
      <view className="shrink min-w-0 gap-0.5">
        <text className="text-body font-medium text-text">7,200 steps</text>
        <text className="text-footnote text-muted">2,800 to your goal of 10,000</text>
      </view>
    </view>
  );
}

/** At the end of a row, where a bar has no room. */
export function InRows() {
  return (
    <ItemList label="Uploads">
      <Item label="Harbour at dawn.jpg" description="4.2 MB" endContent={<CircularProgress label="Uploading Harbour at dawn.jpg" value={64} valueLabel="64%" size="sm" data-testid="row" />} />
      <Item label="Market stalls.jpg" description="3.1 MB" endContent={<CircularProgress label="Uploading Market stalls.jpg" size="sm" data-testid="waiting" />} />
      <Item label="Tram 28.jpg" description="5.6 MB" endContent={<CircularProgress label="Uploaded Tram 28.jpg" value={100} valueLabel="Done" size="sm" tone="success" />} />
    </ItemList>
  );
}

export function Sizes() {
  return (
    <CircularProgressGroup className="flex-row flex-wrap items-center gap-5">
      <CircularProgress label="Storage used" value={40} valueLabel="40%" size="sm" />
      <CircularProgress label="Storage used" value={40} valueLabel="40%" />
      <CircularProgress label="Storage used" value={40} valueLabel="40%" size="lg" />
    </CircularProgressGroup>
  );
}

/** A meter in a tone: the words say how it stands too. */
export function Tones() {
  return (
    <CircularProgressGroup className="flex-row flex-wrap items-center gap-5">
      <CircularProgress label="Monthly goal" value={100} valueLabel="Done" tone="success" size="lg" />
      <CircularProgress label="Storage" value={86} valueLabel="86%" tone="warning" size="lg" />
      <CircularProgress label="Data plan" value={100} valueLabel="Over" tone="danger" size="lg" data-testid="over" />
    </CircularProgressGroup>
  );
}

/** The home tile. */
export function Specimen() {
  return <CircularProgress label="Progress" value={66} valueLabel="66%" />;
}
