import { cx } from '@lingxia/view';
import { Glyph } from '../internal/Glyph';
import { Tooltip } from './Tooltip';
import { tooltipRecipe } from './Tooltip.recipe';

/** Beside the thing it explains. */
export function Storage() {
  return (
    <view className="flex-row items-center gap-1">
      <text className="text-body text-text">Storage used: 9.6 of 10 GB</text>
      <Tooltip label="About storage" content="Attachments count toward storage. Notes themselves do not." data-testid="storage" />
    </view>
  );
}

/** Below the button, where there is no room above. */
export function Below() {
  return (
    <view className="flex-row items-center gap-1">
      <text className="text-body text-text">Two-step sign-in</text>
      <Tooltip label="About two-step sign-in" content="A code from your phone, on top of your password." placement="bottom" data-testid="two-step" />
    </view>
  );
}

export function Sizes() {
  return (
    <view className="flex-row items-center gap-4">
      <Tooltip label="Small" content="Small." size="sm" />
      <Tooltip label="Medium" content="Medium." />
    </view>
  );
}

/** Next to a field label, for what the label cannot say. */
export function InForm() {
  return (
    <view className="gap-1.5">
      <view className="flex-row items-center gap-0.5">
        <text className="text-footnote font-medium text-text">Workspace URL</text>
        <Tooltip label="About the workspace URL" content="People you invite open this address. You can change it once a month." size="sm" data-testid="url" />
      </view>
      <view className="min-h-control-md justify-center px-3 rounded-control border border-line-strong bg-surface">
        <text className="text-body text-text">notes.example.com/atlas</text>
      </view>
    </view>
  );
}

/** A still drawing of an open note over its button. */
export function Parts() {
  return (
    <view className="items-center gap-1 py-4">
      <view className={tooltipRecipe.bubble.base}>
        <text className={tooltipRecipe.text.base}>Attachments count toward storage. Notes themselves do not.</text>
      </view>
      <DrawnTrigger fill="bg-text/6" />
    </view>
  );
}

/** A still trigger, drawn because a view refuses `active:`. */
function DrawnTrigger({ fill }: { fill?: string }) {
  return (
    <view className={cx('size-10 items-center justify-center rounded-full text-muted', fill)}>
      <Glyph name="info" className={tooltipRecipe.icon.size.md} />
    </view>
  );
}

export function StateClosed() {
  return <Tooltip label="About storage" content="Attachments count toward storage." />;
}

export function StatePressed() {
  return <DrawnTrigger fill="bg-text/10" />;
}

export function StateOpen() {
  return (
    <view className="items-center gap-1">
      <view className={tooltipRecipe.bubble.base}>
        <text className={tooltipRecipe.text.base}>A short note</text>
      </view>
      <DrawnTrigger fill="bg-text/6" />
    </view>
  );
}

/** The home tile: the still drawing, small. */
export function Specimen() {
  return (
    <view className="items-center gap-1">
      <view className={tooltipRecipe.bubble.base}>
        <text className={tooltipRecipe.text.base}>Counts attachments</text>
      </view>
      <view className="size-8 items-center justify-center rounded-full text-muted">
        <Glyph name="info" className={tooltipRecipe.icon.size.sm} />
      </view>
    </view>
  );
}

/** Beside its button, where the note would cover what is above and below. */
export function Beside() {
  return (
    <view className="flex-row items-center gap-1">
      <text className="text-body text-text">Sync</text>
      <Tooltip label="About sync" content="Notes are sent when you are online." placement="end" size="sm" data-testid="side" />
    </view>
  );
}
