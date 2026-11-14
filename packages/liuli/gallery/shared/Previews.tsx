import { Badge } from '@liulidesign/core/Badge';
import { Button } from '@liulidesign/core/Button';
import { ProgressBar } from '@liulidesign/core/ProgressBar';
import { SegmentedControl } from '@liulidesign/core/SegmentedControl';
import { Slider } from '@liulidesign/core/Slider';
import { Switch } from '@liulidesign/core/Switch';
import { Tabs } from '@liulidesign/core/Tabs';
import { TextInput } from '@liulidesign/core/TextInput';
import { ToggleButton } from '@liulidesign/core/ToggleButton';
import { Avatar } from '@liulidesign/core/Avatar';
import { Item, ItemList } from '@liulidesign/core/Item';
import { Specimen as DialogSpecimen } from '../../core/src/Dialog/Dialog.examples';
import { SignInSpecimen, TypographySpecimen, ColourSpecimen } from './Specimens';

// One preview per home category card: one or two of its real components, small and still, centred
// on the tile's stage (about 160 by 88 inside its inset on a phone). Named after the category id;
// generate.mjs checks each exists.

const none = () => {};

export function FoundationsPreview() {
  return (
    <view className="items-center gap-3">
      <TypographySpecimen />
      <ColourSpecimen />
    </view>
  );
}

export function ActionsPreview() {
  return (
    <view className="items-center gap-2.5">
      <Button label="Save" variant="primary" size="sm" />
      <ToggleButton label="Bold" size="sm" isPressed onPressedChange={none} />
    </view>
  );
}

export function InputsPreview() {
  return (
    <view className="w-full gap-3">
      <TextInput value="Ana Lima" onChange={none} label="Name" isLabelHidden size="sm" />
      <Slider value={60} onChange={none} label="Volume" isLabelHidden size="sm" />
    </view>
  );
}

export function SelectionPreview() {
  return (
    <view className="w-full gap-2.5">
      <Switch value onChange={none} label="Wi-Fi" size="sm" />
      <SegmentedControl label="Layout" options={[{ value: 'list', label: 'List' }, { value: 'grid', label: 'Grid' }]} value="list" onChange={none} size="sm" />
    </view>
  );
}

const ranges = [{ value: 'day', label: 'Day' }, { value: 'week', label: 'Week' }];

export function NavigationPreview() {
  return <Tabs label="Range" items={ranges} value="week" onChange={none} className="w-full" />;
}

export function FeedbackPreview() {
  return (
    <view className="items-center gap-3">
      <view className="flex-row gap-1.5">
        <Badge label="Paid" tone="success" />
        <Badge label="New" tone="accent" emphasis="solid" />
      </view>
      <ProgressBar label="Uploading" isLabelHidden value={62} className="w-24" />
    </view>
  );
}

export function OverlaysPreview() {
  return <DialogSpecimen />;
}

export function ContentPreview() {
  return (
    <ItemList className="w-full">
      <Item label="Ana Lima" description="Editor" startContent={<Avatar name="Ana Lima" size="sm" />} />
    </ItemList>
  );
}

/** A still frame: the media surface with a play mark, so the home page loads no video. The hairline keeps the frame visible on a dark stage. */
export function MediaPreview() {
  return (
    <view className="aspect-video w-28 items-center justify-center overflow-hidden rounded-lg border border-text/12 bg-media">
      <view className="size-8 items-center justify-center rounded-full border border-on-media/20 bg-media">
        <vector aria-hidden paths={[{ d: 'M9 7.5v9l7.5-4.5z', fill: 'currentColor' }]} viewBox={[0, 0, 24, 24]} className="size-4 text-on-media" />
      </view>
    </view>
  );
}

export function PatternsPreview() {
  return <SignInSpecimen />;
}
