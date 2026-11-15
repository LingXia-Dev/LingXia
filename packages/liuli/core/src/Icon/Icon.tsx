import { createContext, cx, useContext, type VectorPath } from '@lingxia/view';
import type { IconDefinition, IconSet } from './definition';
import { iconRecipe as r } from './Icon.recipe';
import type { IconName } from './names';
import { lucide } from './set';

export type { IconDefinition, IconSet } from './definition';
export { iconNames, type IconName } from './names';

export type IconSize = 'sm' | 'md' | 'lg';

export type IconSource =
  | {
      /** The icon by what it means: `close`, `search`, `chevron-end`. The set in use decides how it is drawn. */
      name: IconName;
      icon?: never;
    }
  | {
      name?: never;
      /** An icon of the app's own, or one module from a set, drawn as given whatever set is in use. */
      icon: IconDefinition;
    };

export type IconProps = IconSource & {
  /** 16, 20 or 24 points. @default 'md' */
  size?: IconSize;
  /** What the icon says, when nothing beside it does. Without it the icon is decoration and is not read. */
  label?: string;
  className?: string;
  'data-testid'?: string;
};

const grid = [0, 0, 24, 24] as const;
// 1.8 units on the 24-unit grid is a 1.5 px stroke at 20 px.
const stroke = { fill: 'none', stroke: 'currentColor', strokeWidth: 1.8, strokeLinecap: 'round', strokeLinejoin: 'round' } as const;

export const iconGrid = grid;

// A definition's paths, made once per stroke weight: icons repeat on a page and are redrawn with it.
const drawn = new WeakMap<IconDefinition, Map<number, VectorPath[]>>();

/** An icon's paths on the 24-unit grid, stroked in the current colour; `weight` is the stroke in grid units. */
export function iconPaths(icon: IconDefinition, weight: number = stroke.strokeWidth): VectorPath[] {
  let weights = drawn.get(icon);
  if (!weights) drawn.set(icon, (weights = new Map()));
  let paths = weights.get(weight);
  if (!paths) weights.set(weight, (paths = icon.paths.map(path => (typeof path === 'string' ? { d: path, ...stroke, strokeWidth: weight } : path))));
  return paths;
}

const Icons = createContext<Partial<IconSet>>(lucide);

/** The icon a name draws in the set in use; a name the set leaves out comes from the default set, so a name never draws blank. */
export function useIcon(name: IconName): IconDefinition {
  return useContext(Icons)[name] ?? lucide[name];
}

/** One icon from the set in use, in the colour of the text around it. */
export function Icon(props: IconProps) {
  const { size = 'md', label, className, 'data-testid': testId } = props;
  const named = useIcon(props.name ?? 'close');
  const icon = props.icon ?? named;
  const mirrored = icon.isMirrored && r.glyph.mirrored;
  if (label === undefined)
    return <vector aria-hidden data-testid={testId} paths={iconPaths(icon)} viewBox={grid} className={cx(r.glyph.base, r.glyph.size[size], mirrored, className)} />;
  return (
    <view role="img" aria-label={label} data-testid={testId} className={cx(r.box.base, r.box.size[size], className)}>
      <vector aria-hidden paths={iconPaths(icon)} viewBox={grid} className={cx(r.glyph.base, r.glyph.size[size], mirrored)} />
    </view>
  );
}

export interface IconSetProviderProps {
  /** The icons to draw for the names they cover; the rest stay as the default set draws them. */
  icons: Partial<IconSet>;
  /** One element; every Liuli component inside it draws from `icons`. */
  children: JSX.Element;
}

/** Changes how icons are drawn inside it: a theme's own set, or one icon swapped. */
export function IconSetProvider({ icons, children }: IconSetProviderProps) {
  return <Icons.Provider value={icons}>{children}</Icons.Provider>;
}
