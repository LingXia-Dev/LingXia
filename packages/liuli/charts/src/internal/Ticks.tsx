import { cx, useBox, type Child } from '@lingxia/view';
import { chartRecipe as r } from '../Chart/Chart.recipe';
import { fitPlaced, fitTicks } from './geometry';

interface TicksProps {
  /** Every category's label, in data order. */
  texts: readonly string[];
  /** `points` spreads the data edge to edge inside `inset`; `slots` centres each in an equal share. */
  layout: 'points' | 'slots';
  inset?: number;
  /** With `points`, each label's place from 0 to 1 instead of an even spread. */
  places?: readonly number[];
  /** What to draw until the row has been measured. */
  children?: Child;
}

// The least space between two labels.
const gap = 8;

/**
 * The category labels that fit, each under its datum. The row and its longest label are measured
 * after layout, so the first frame draws `children` and the fitted labels follow.
 */
export function Ticks({ texts, layout, inset = 0, places, children }: TicksProps) {
  const row = useBox('ticks');
  const widest = useBox('tick');
  const longest = texts.reduce((a, b) => (b.length > a.length ? b : a), '');
  const width = row.value?.width ?? 0;
  const tick = widest.value?.width ?? 0;
  const isMeasured = width > 0 && tick > 0;
  const count = texts.length;
  const pitch = layout === 'points' ? (width - inset * 2) / Math.max(1, count - 1) : width / Math.max(1, count);
  const center = (index: number) => (places ? inset + places[index] * (width - inset * 2) : layout === 'points' ? (count > 1 ? inset + index * pitch : width / 2) : (index + 0.5) * pitch);
  const left = (index: number) => Math.round(Math.min(Math.max(0, width - tick), Math.max(0, center(index) - tick / 2)) * 2) / 2;
  // A label at an edge is held inside the row, up to half its width nearer its neighbour.
  const held = Math.max(0, tick / 2 - (layout === 'points' ? inset : pitch / 2));
  const fitted = !isMeasured ? [] : places ? fitPlaced(places, width - inset * 2, tick + gap) : fitTicks(count, pitch, tick + gap + held, layout === 'points' ? 'end' : 'start');
  return (
    <view box={row} className={r.ticks.base}>
      <text box={widest} className={cx(r.tick.base, r.tick.ghost)}>{longest}</text>
      {!isMeasured ? children : fitted.map(index => {
        // A label held in by an edge lines up with that edge, not with the middle of the widest label.
        const align = center(index) < tick / 2 ? 'start' : center(index) > width - tick / 2 ? 'end' : 'center';
        return <text key={String(index)} className={cx(r.tick.base, r.tick.placed, r.tick.align[align])} style={{ left: left(index), width: tick }}>{texts[index]}</text>;
      })}
    </view>
  );
}
