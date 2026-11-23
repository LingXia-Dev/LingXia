import { cx, useView } from '@lingxia/view';
import { iconGrid, iconPaths, useIcon, type IconRef } from '../Icon/Icon';
import { iconRecipe } from '../Icon/Icon.recipe';

/**
 * An icon inside a component: drawn from the set in use like `Icon`, but sized and coloured by
 * the component's own recipe, and never read. `weight` is the stroke on the 24-unit grid, for a
 * mark that must stay solid when small (a tick in a checkbox).
 */
export function Glyph({ name, weight, className, side, cap = 1.5 }: {
  name: IconRef; weight?: number; className?: string;
  /** The icon's side in points at the default text size, for an icon beside text: it then grows with the text, up to half again. */
  side?: number;
  /** How far `side` grows; a glyph in a tile grows as the tile does. */
  cap?: number;
}) {
  const fromSet = useIcon(typeof name === 'string' ? name : 'close');
  const icon = typeof name === 'string' ? fromSet : name;
  const grown = useTextGrowth(side, cap);
  return <vector aria-hidden paths={iconPaths(icon, weight)} viewBox={iconGrid} style={grown} className={cx(icon.isMirrored && iconRecipe.glyph.mirrored, className)} />;
}

/** A box of `side` points grown with the text size up to `cap` times, or nothing at the default size. */
export function useTextGrowth(side: number | undefined, cap: number): { width: number; height: number } | undefined {
  const scale = useView().env.textScale ?? 1;
  if (side === undefined || scale <= 1) return undefined;
  const grown = Math.round(side * Math.min(scale, cap));
  return { width: grown, height: grown };
}
