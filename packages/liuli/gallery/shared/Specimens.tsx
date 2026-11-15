import { MessageBubble } from '@liulidesign/core/MessageBubble';
import { Button } from '@liulidesign/core/Button';
import { Stat } from '@liulidesign/charts/Stat';
import { Item, ItemList } from '@liulidesign/core/Item';
import { ProgressBar } from '@liulidesign/core/ProgressBar';
import { Switch } from '@liulidesign/core/Switch';
import { TextInput } from '@liulidesign/core/TextInput';

// Tile specimens for the Foundations and Patterns pages, one per page, named after it. Patterns
// show their own components at small scale; foundations draw the tokens themselves.

const none = () => {};

export function ColourSpecimen() {
  return (
    <view className="flex-row gap-3">
      <view className="size-5.5 rounded-full bg-accent" />
      <view className="size-5.5 rounded-full bg-success" />
      <view className="size-5.5 rounded-full bg-warning" />
      <view className="size-5.5 rounded-full bg-danger" />
    </view>
  );
}

export function TypographySpecimen() {
  return (
    <view className="flex-row items-baseline gap-2">
      <text className="text-display font-semibold tracking-tight text-text">Aa</text>
      <text className="text-title text-muted">Aa</text>
      <text className="text-body text-faint">Aa</text>
    </view>
  );
}

export function ShapeAndSpacingSpecimen() {
  return (
    <view className="flex-row items-center gap-2">
      <view className="size-5 rounded-checkbox border border-accent/40 bg-accent-soft" />
      <view className="size-8 rounded-control border border-accent/40 bg-accent-soft" />
      <view className="size-12 rounded-container border border-accent/40 bg-accent-soft" />
    </view>
  );
}

/**
 * The two elevations at tile scale: the real shadows are wider than a 96 px stage, so these are the
 * same two-layer shapes scaled down, lifted so the modal's falls inside the stage.
 */
export function ElevationSpecimen() {
  return (
    <view className="mb-5 flex-row items-end gap-3">
      <view className="w-12 h-9 rounded-control border border-text/12 bg-surface-overlay shadow-[0_1px_2px_rgb(0_0_0/0.06),0_3px_8px_rgb(0_0_0/0.08)]" />
      <view className="w-14 h-11 rounded-container border border-text/12 bg-surface-overlay shadow-[0_2px_4px_rgb(0_0_0/0.10),0_10px_22px_rgb(0_0_0/0.22)]" />
    </view>
  );
}

/** One dot's path as a still: spacing shrinks along the trail, as an ease-out move does. */
export function MotionSpecimen() {
  return (
    <view className="flex-row items-center">
      <view className="size-4 rounded-full bg-accent/30" />
      <view className="ms-6 size-4 rounded-full bg-accent/45" />
      <view className="ms-3 size-4 rounded-full bg-accent/60" />
      <view className="ms-1.5 size-4 rounded-full bg-accent/80" />
      <view className="ms-0.5 size-4 rounded-full bg-accent" />
    </view>
  );
}

export function SignInSpecimen() {
  return (
    <view className="w-full gap-2">
      <TextInput value="ana@example.com" onChange={none} label="Email" isLabelHidden size="sm" />
      <Button label="Sign in" variant="primary" size="sm" width="fill" />
    </view>
  );
}

export function SettingsSpecimen() {
  return (
    <ItemList className="w-full">
      <Switch value onChange={none} label="Wi-Fi" variant="row" />
    </ItemList>
  );
}

export function ListDetailSpecimen() {
  return (
    <ItemList className="w-full">
      <Item label="Two more seats" description="Ana Lima" isSelected onClick={none} />
    </ItemList>
  );
}

export function DashboardSpecimen() {
  return (
    <view className="w-full rounded-container border border-text/12 bg-surface p-3">
      <Stat label="Revenue" value="€34,020" delta="8.8%" deltaDirection="up" deltaLabel="Up 8.8%" size="sm" />
    </view>
  );
}

export function ConversationSpecimen() {
  return (
    <view className="w-full gap-1 rounded-container border border-text/12 bg-surface p-3">
      <MessageBubble text="Saturday?" />
      <MessageBubble side="end" text="Yes!" />
    </view>
  );
}

export function UploadsSpecimen() {
  return (
    <view className="w-full gap-2 rounded-container border border-text/12 bg-surface p-3">
      <ProgressBar label="photos.zip" valueLabel="64%" size="sm" value={64} />
    </view>
  );
}
