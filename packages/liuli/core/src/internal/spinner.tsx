import { type ColorValue, type VectorPath } from '@lingxia/view';
import { useTextGrowth } from './Glyph';
import { spinnerRecipe as r } from './spinner.recipe';

const box = [0, 0, 24, 24] as const;
const stroke = { fill: 'none', stroke: 'currentColor', strokeWidth: 2.5, strokeLinecap: 'round', strokeLinejoin: 'round' } as const;
// The whole ring, faint, so a still frame reads as a spinner rather than a stray tick. A translucent role, not
// opacity, which would composite a layer for every spinner.
const ringOf = (ring: ColorValue): VectorPath[] => [{ d: 'M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18', ...stroke, stroke: ring }];
const rings = new Map<ColorValue, VectorPath[]>();
const arc: VectorPath[] = [{ d: 'M21 12a9 9 0 0 0-9-9', ...stroke }];

/**
 * A quarter arc in the colour around it over its faint ring, spun by `animate-spin` in `className`; the ring
 * looks the same at any angle, so the whole mark turns. A motion mark, not an icon: no icon set redraws it. Hidden from
 * assistive tech; the control around it reports busy.
 */
export function SpinnerMark({ className, style, side, ring = 'text/15' }: {
  className?: string;
  style?: { width?: number; height?: number; marginTop?: number };
  /** Its side in points at the default text size, beside text: it grows with the text as a glyph does. */
  side?: number;
  /** The ring's colour role: on a solid fill, that fill's on-colour, faint (`on-accent-solid/30`). @default 'text/15' */
  ring?: ColorValue;
}) {
  let track = rings.get(ring);
  if (!track) rings.set(ring, track = ringOf(ring));
  const grown = useTextGrowth(side, 1.5);
  return (
    <view aria-hidden style={grown ? { ...style, ...grown } : style} className={className}>
      <vector paths={track} viewBox={box} className={r.track} />
      <vector paths={arc} viewBox={box} className={r.arc} />
    </view>
  );
}
