import { IconButton } from '../IconButton/IconButton';
import { Glyph } from '../internal/Glyph';
import { tooltipRecipe } from './Tooltip.recipe';
import { Tooltip } from './Tooltip';
import { Link } from '../Link/Link';

/** A toolbar of icon buttons: each hint says the button's name in words. */
export function Toolbar() {
  return (
    <view className="flex-row items-center gap-1">
      <Tooltip content="Undo" data-testid="undo"><IconButton label="Undo" icon="undo" data-testid="undo" /></Tooltip>
      <Tooltip content="Redo"><IconButton label="Redo" icon="redo" /></Tooltip>
      <Tooltip content="Share"><IconButton label="Share" icon="share" /></Tooltip>
    </view>
  );
}

/** Below the trigger, where there is no room above. */
export function Below() {
  return (
    <view className="flex-row">
      <Tooltip content="Print" placement="bottom" data-testid="print"><IconButton label="Print" icon="print" data-testid="print" /></Tooltip>
    </view>
  );
}

/** Words as the trigger: wrapped in one plain button, a single focus stop. */
export function OnWords() {
  return (
    <view className="flex-row">
      <Tooltip content="Last synced 2 minutes ago" data-testid="synced"><text className="text-footnote text-muted">Synced</text></Tooltip>
    </view>
  );
}

/** The hint drawn at rest, for the gallery's parts stage. */
export function Parts() {
  return (
    <view className="items-start">
      <view className={tooltipRecipe.bubble.base}><text className={tooltipRecipe.text.base}>Undo</text></view>
    </view>
  );
}

/** The home tile: a still drawing, since a real hint opens over the page. */
export function Specimen() {
  return (
    <view className="items-center gap-1">
      <view className="px-2 py-1 rounded-control bg-text"><text className="text-caption text-surface">Undo</text></view>
      <view className="size-6 items-center justify-center rounded-control bg-text/6"><Glyph name="undo" className="size-3.5 text-muted" /></view>
    </view>
  );
}

/** On a link in a sentence: the hint spells out the short name the link shows. */
export function InSentence() {
  return (
    <text className="text-body text-text">
      Shipped by{' '}
      <Tooltip content="United Parcel Service">
        <Link label="UPS" data-testid="carrier-link" />
      </Tooltip>
      {' '}this morning.
    </text>
  );
}
