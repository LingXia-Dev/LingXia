import { useLocal } from '@lingxia/view';
import { Button } from '../Button/Button';
import { Switch } from '../Switch/Switch';
import { popoverRecipe } from './Popover.recipe';
import { Popover } from './Popover';

/** View options for a list: switches that apply at once, in a panel under the trigger. */
export function ViewOptions() {
  const previews = useLocal('previews', true);
  const dates = useLocal('dates', false);
  return (
    <view className="gap-3">
      <Popover label="View options" title="Show in the list" data-testid="options">
        <Switch bind={previews} label="Previews" size="sm" data-testid="previews" />
        <Switch bind={dates} label="Dates" size="sm" data-testid="dates" />
      </Popover>
      <text className="text-footnote text-muted" data-testid="summary">
        {[previews.value && 'Previews', dates.value && 'Dates'].filter(Boolean).join(' and ') || 'Titles only'}
      </text>
    </view>
  );
}

/** Sharing a note: the link and one action, opening above a trigger that sits low on the page. */
export function Share() {
  const copied = useLocal('copied', false);
  return (
    <view className="items-end">
      <Popover label="Share" title="Anyone with the link can view" placement="top" alignment="end" variant="primary" data-testid="share">
        <text className="text-footnote text-muted select-text">notes.example.com/s/q3-plan</text>
        <Button label={copied.value ? 'Copied' : 'Copy link'} size="sm" width="content" onClick={() => copied.set(true)} data-testid="copy" />
      </Popover>
    </view>
  );
}

/** The panel drawn at rest under its trigger, for the gallery's parts stage: its own part classes, in the page. */
export function Parts() {
  return (
    <view className="items-start gap-1.5">
      <Button label="View options" />
      <view className="w-72 p-4 gap-3 rounded-container border border-text/12 bg-surface-overlay shadow-raised">
        <text className={popoverRecipe.title.base}>Show in the list</text>
        <text className="text-body text-text">Previews</text>
        <text className="text-body text-text">Dates</text>
      </view>
    </view>
  );
}

/** The home tile: a still drawing, since a real panel opens over the page. */
export function Specimen() {
  return (
    <view className="items-start gap-1">
      <view className="min-h-6 justify-center rounded-md border border-text/12 bg-surface px-2"><text className="text-caption font-medium text-text">Options</text></view>
      <view className="w-28 p-2 gap-1 rounded-control border border-text/12 bg-surface-overlay shadow-[0_1px_2px_rgb(0_0_0/0.06),0_4px_12px_rgb(0_0_0/0.10)]">
        <text className="text-caption font-semibold text-text">Show</text>
        <view className="h-1.5 w-3/4 rounded-full bg-text/10" />
        <view className="h-1.5 w-1/2 rounded-full bg-text/10" />
      </view>
    </view>
  );
}

/** Beside its trigger: a tool at the edge of a canvas opens its options at its side. */
export function Beside() {
  return (
    <view className="items-start gap-2">
      <Popover label="Line" title="Line" placement="end" data-testid="beside">
        <text className="text-footnote text-muted">Weight and style for the shapes you draw next.</text>
      </Popover>
    </view>
  );
}
