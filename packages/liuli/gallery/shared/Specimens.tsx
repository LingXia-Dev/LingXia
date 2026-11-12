import { ProgressBar } from '@liulidesign/core/ProgressBar';

// Home tile drawings for the Foundations and Patterns pages, one per page, named after it.

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
      <view className="w-12 h-9 rounded-control border border-text/8 bg-surface-raised shadow-[0_1px_2px_rgb(0_0_0/0.06),0_3px_8px_rgb(0_0_0/0.08)]" />
      <view className="w-14 h-11 rounded-container border border-text/8 bg-surface-raised shadow-[0_2px_4px_rgb(0_0_0/0.10),0_10px_22px_rgb(0_0_0/0.22)]" />
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
    <view className="w-28 gap-1.5">
      <view className="h-5 rounded-md border border-line-strong bg-surface" />
      <view className="h-5 rounded-md border border-line-strong bg-surface" />
      <view className="h-5 rounded-md bg-accent" />
    </view>
  );
}

export function SettingsSpecimen() {
  return (
    <view className="w-28 rounded-control border border-text/10 bg-surface divide-y divide-text/8">
      <view className="h-5 flex-row items-center px-2"><view className="w-12 h-1.5 rounded-full bg-text/25" /></view>
      <view className="h-5 flex-row items-center justify-between px-2">
        <view className="w-9 h-1.5 rounded-full bg-text/25" />
        <view className="w-5 h-2.5 rounded-full bg-success-soft" />
      </view>
      <view className="h-5 flex-row items-center px-2"><view className="w-14 h-1.5 rounded-full bg-danger/50" /></view>
    </view>
  );
}

export function UploadsSpecimen() {
  return (
    <view className="w-28 gap-2 rounded-control border border-text/10 bg-surface p-2.5">
      <view className="w-16 h-1.5 rounded-full bg-text/25" />
      <ProgressBar label="Upload" isLabelHidden size="sm" value={64} />
    </view>
  );
}
