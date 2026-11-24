import { cx, useBox, type VectorPath } from '@lingxia/view';
import { chartRecipe as r } from '../Chart/Chart.recipe';
import type { ChartReference } from './data';
import { placeReferences, type Extent, type ReferencePlace } from './geometry';

// The space between a label and its line, in points: `pb-0.5` and `pt-0.5`.
const gap = 2;

export interface ReferenceTexts { full: string; short: string; spoken: string }

/**
 * Reference labels' texts and measured sizes, and their places once measured: each clear of the
 * others, of `taken`, and of the plot's edges. Before the first layout every label sits above its line.
 */
export function useReferences(
  references: readonly ChartReference[], format: (value: number) => string, lines: (value: number) => number,
  plot: { width: number; height: number } | null, taken: readonly Extent[] = [],
) {
  const texts: ReferenceTexts[] = references.map(each => ({
    full: `${each.label} ${format(each.value)}`, short: format(each.value), spoken: `${each.label}, ${format(each.value)}`,
  }));
  const boxes = texts.map((_, index) => [useBox(`reference-${index}`), useBox(`reference-${index}-value`)] as const);
  const height = boxes[0]?.[0].value?.height ?? 0;
  const places: ReferencePlace[] = plot && height > 0
    ? placeReferences(references.map(each => lines(each.value)), plot, {
      full: boxes.map(([full]) => full.value?.width ?? 0), short: boxes.map(([, short]) => short.value?.width ?? 0), height,
    }, taken, gap)
    : texts.map(() => ({ side: 'above', inset: 0, isShort: false }));
  const ghost = texts.length > 0 && (
    <view aria-hidden className={r.references.ghost}>
      {texts.flatMap((text, index) => [
        <text key={`full-${index}`} box={boxes[index][0]} className={r.referenceLabel.measure}>{text.full}</text>,
        <text key={`short-${index}`} box={boxes[index][1]} className={r.referenceLabel.measure}>{text.short}</text>,
      ])}
    </view>
  );
  return { texts, places, ghost };
}

/** Each label at its line: `shares` are the lines' heights as percentages of the box they sit in. */
export function ReferenceLabels({ texts, places, shares, frame }: {
  texts: readonly ReferenceTexts[]; places: readonly ReferencePlace[]; shares: readonly number[];
  /** The box the shares are of, over the plot. @default the whole box it is in */
  frame?: string;
}) {
  if (!texts.length) return null;
  return (
    <view aria-hidden className={frame ?? r.references.frame}>
      <view className={r.references.base}>
      {texts.map((text, index) => (
        <view key={`reference-${index}`} className={r.referenceMover.base} style={{ translate: [0, `${shares[index]}%`] }}>
          <view className={r.referenceAnchor.base}>
            <text className={cx(r.referenceLabel.base, r.referenceLabel[places[index].side])} style={{ insetInlineEnd: places[index].inset }}>{places[index].isShort ? text.short : text.full}</text>
          </view>
        </view>
      ))}
      </view>
    </view>
  );
}

/** What each reference says, for assistive tech, after the values. */
export function ReferenceReading({ texts }: { texts: readonly ReferenceTexts[] }) {
  if (!texts.length) return null;
  return <view className={r.referenceReading.base}>{texts.map((text, index) => <text key={`reference-${index}`}>{text.spoken}</text>)}</view>;
}

/** A dashed hairline across the drawing at height `y`, from `from` to `to` in its units, broken at each of `gaps`. */
export const referencePath = (index: number, y: number, from: number, to: number, gaps: ReadonlyArray<readonly [number, number]> = []): VectorPath => ({
  key: `reference-${index}`, d: segments(y, from, to, gaps), stroke: 'text-muted', strokeWidth: 1, strokeDasharray: [4, 3], fill: 'none', vectorEffect: 'non-scaling-stroke',
});

const round = (value: number) => Math.round(value * 100) / 100;
const segments = (y: number, from: number, to: number, gaps: ReadonlyArray<readonly [number, number]>) => {
  let d = `M${from} ${y}`;
  let at = from;
  for (const [start, end] of gaps) {
    if (end <= at || start >= to) continue;
    if (start > at) d += `H${round(start)}`;
    at = Math.min(to, end);
    d += `M${round(at)} ${y}`;
  }
  return at < to ? `${d}H${to}` : d.slice(0, d.lastIndexOf('M'));
};
