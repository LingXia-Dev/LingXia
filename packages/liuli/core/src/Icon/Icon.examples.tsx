import { Icon, IconSetProvider, iconNames, type IconDefinition } from './Icon';

/** Icons are named by what they mean, so a theme can redraw them without touching this code. */
export function Common() {
  return (
    <view className="flex-row flex-wrap gap-5 text-text">
      <Icon name="search" data-testid="search" />
      <Icon name="add" />
      <Icon name="edit" />
      <Icon name="share" />
      <Icon name="delete" />
      <Icon name="settings" />
      <Icon name="more" />
    </view>
  );
}

export function Sizes() {
  return (
    <view className="flex-row items-center gap-5 text-text">
      <Icon name="calendar" size="sm" data-testid="small" />
      <Icon name="calendar" data-testid="medium" />
      <Icon name="calendar" size="lg" data-testid="large" />
    </view>
  );
}

/** `inline` takes its size from the text around it: larger in a title, smaller in a footnote. */
export function Inline() {
  return (
    <view className="gap-3 text-text">
      <view className="flex-row items-center gap-2 text-title">
        <Icon name="calendar" size="inline" data-testid="in-title" />
        <text className="text-title font-semibold">Planning</text>
      </view>
      <view className="flex-row items-center gap-1.5 text-footnote text-muted">
        <Icon name="clock" size="inline" data-testid="in-footnote" />
        <text className="text-footnote text-muted">Updated an hour ago</text>
      </view>
    </view>
  );
}

/** An icon takes the colour of the text around it. */
export function Colours() {
  return (
    <view className="gap-3">
      <view className="flex-row items-center gap-2 text-text">
        <Icon name="document" />
        <text className="text-body">Quarterly report</text>
      </view>
      <view className="flex-row items-center gap-2 text-muted">
        <Icon name="clock" size="sm" />
        <text className="text-footnote">Edited 2 hours ago</text>
      </view>
      <view className="flex-row items-center gap-2 text-danger">
        <Icon name="error" size="sm" data-testid="tinted" />
        <text className="text-footnote">Upload failed</text>
      </view>
      <view className="flex-row items-center gap-2 text-text/40">
        <Icon name="close" size="sm" data-testid="faded" />
        <text className="text-footnote">Cancelled</text>
      </view>
    </view>
  );
}

/** Alone, an icon needs a label to be read; beside text that says the same, it stays silent. */
export function Labelled() {
  return (
    <view className="flex-row items-center gap-5">
      <Icon name="lock" label="Private" className="text-muted" data-testid="private" />
      <view className="flex-row items-center gap-1.5 text-success">
        <Icon name="success" size="sm" data-testid="silent" />
        <text className="text-footnote font-medium">Paid</text>
      </view>
    </view>
  );
}

/** Icons that point along the reading direction turn round in a right-to-left layout; the rest do not. */
export function Mirrored() {
  return (
    <view className="gap-3 text-text">
      <view className="flex-row items-center gap-5">
        <Icon name="chevron-end" data-testid="forward" />
        <Icon name="arrow-end" />
        <Icon name="send" />
        <Icon name="search" />
      </view>
      <view dir="rtl" className="flex-row items-center gap-5">
        <Icon name="chevron-end" data-testid="forward-rtl" />
        <Icon name="arrow-end" />
        <Icon name="send" />
        <Icon name="search" />
      </view>
    </view>
  );
}

const bolt: IconDefinition = { paths: ['M13 2 4.5 13.5H11L10 22l8.5-11.5H12z'] };
const cross: IconDefinition = { paths: ['M7 7l10 10M17 7 7 17', 'M3 12a9 9 0 1 0 18 0a9 9 0 1 0 -18 0z'] };

/** An icon of the app's own, and a set that redraws one name for everything inside it. */
export function Custom() {
  return (
    <view className="flex-row items-center gap-5 text-text">
      <Icon icon={bolt} data-testid="own" />
      <Icon name="close" data-testid="default-close" />
      <IconSetProvider icons={{ close: cross }}>
        <view className="flex-row items-center gap-5">
          <Icon name="close" data-testid="swapped-close" />
          <Icon name="search" data-testid="kept-search" />
        </view>
      </IconSetProvider>
    </view>
  );
}

const half = Math.ceil(iconNames.length / 2);

/** Every name in the set, in two columns. */
export function Everything() {
  return (
    <view className="flex-row gap-4">
      {[iconNames.slice(0, half), iconNames.slice(half)].map((column, index) => (
        <view key={String(index)} className="min-w-0 grow basis-0 gap-3">
          {column.map(name => (
            <view key={name} className="min-w-0 flex-row items-center gap-2.5">
              <Icon name={name} className="text-text" />
              <text className="min-w-0 grow basis-0 truncate text-footnote text-muted">{name}</text>
            </view>
          ))}
        </view>
      ))}
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <view className="flex-row items-center gap-4 text-text">
      <Icon name="search" size="lg" />
      <Icon name="heart" size="lg" />
      <Icon name="settings" size="lg" />
    </view>
  );
}
