import { cx } from '@lingxia/view';
import { iconGrid, iconPaths, useIcon, type IconName } from '../Icon/Icon';
import { iconRecipe } from '../Icon/Icon.recipe';

/**
 * An icon inside a component: drawn from the set in use like `Icon`, but sized and coloured by
 * the component's own recipe, and never read. `weight` is the stroke on the 24-unit grid, for a
 * mark that must stay solid when small (a tick in a checkbox).
 */
export function Glyph({ name, weight, className }: { name: IconName; weight?: number; className?: string }) {
  const icon = useIcon(name);
  return <vector aria-hidden paths={iconPaths(icon, weight)} viewBox={iconGrid} className={cx(icon.isMirrored && iconRecipe.glyph.mirrored, className)} />;
}
