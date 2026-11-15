import { createContext, useContext } from '@lingxia/view';

/** How closely a screen is packed: `compact` for a desk with a pointer and much to show, `comfortable` everywhere else. */
export type Density = 'comfortable' | 'compact';

const DensityContext = createContext<Density>('comfortable');

export interface DensityProviderProps {
  value: Density;
  /** One element: the part of the screen that takes the density. */
  children: JSX.Element;
}

/**
 * Sets the density for everything inside it. In `compact`, form controls built on Field default to
 * their small size, so a control's label and value step down one size with it; Item, ItemList and
 * Table take their compact rows, whose text does not change. A component's own `size` or `density`
 * still wins. For dense desktop screens with a pointer, not a phone default.
 */
export function DensityProvider({ value, children }: DensityProviderProps) {
  return <DensityContext.Provider value={value}>{children}</DensityContext.Provider>;
}

/** The density of the part of the screen this component is in. */
export const useDensity = (): Density => useContext(DensityContext);

/** A form control's size: its own, else small where the screen is compact. */
export function useControlSize<Size extends 'sm' | 'md' | 'lg'>(own: Size | undefined): Size | 'sm' | 'md' {
  const density = useDensity();
  return own ?? (density === 'compact' ? 'sm' : 'md');
}
