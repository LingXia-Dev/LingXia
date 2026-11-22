import { cx, reveal, useId } from '@lingxia/view';
import { liuliText } from '../i18n/text';
import { Button } from '../Button/Button';
import { tourRecipe as r } from './Tour.recipe';
import { dataOf } from '../internal/data';

export type TourPlacement = 'bottom' | 'top' | 'end' | 'start';

export interface TourStep {
  /** The `id` of the element this step points at. */
  target: string;
  /** What the target is called, for assistive tech: "The New note button". */
  targetLabel: string;
  title: string;
  /** One or two sentences. */
  text: string;
  /** Which side of the target the callout sits on; beside it, it flips for room. @default 'bottom' */
  placement?: TourPlacement;
}

export interface TourProps {
  steps: readonly TourStep[];
  /** The step in view, from 0, or null while the tour is not running. The app holds it. */
  value: number | null;
  /** Runs with the next step to show, and with null when the tour ends: finished, or skipped. */
  onChange: (step: number | null) => void;
  /** Runs when the last step\'s button is pressed, before the tour ends. */
  onFinish?: () => void;
  /** "Next". Defaults to "Next" in the page's language. */
  nextLabel?: string;
  /** "Back". Defaults to "Back" in the page's language. */
  backLabel?: string;
  /** "Skip": ends the tour from any step but the last. Defaults to "Skip" in the page's language. */
  skipLabel?: string;
  /** "Done": on the last step. Defaults to "Done" in the page's language. */
  doneLabel?: string;
  /** Where the tour is, written by the app: `(step, count) => \`${step + 1} of ${count}\``. Defaults to "2 of 5" in the page's language. */
  stepLabel?: (step: number, count: number) => string;
  /** On the callout. */
  'data-testid'?: string;
}

const areas = { bottom: 'bottom', top: 'top', end: 'inline-end', start: 'inline-start' } as const satisfies Record<TourPlacement, string>;
const sides = ['bottom', 'top', 'end', 'start'] as const satisfies readonly TourPlacement[];

/** A few callouts, one at a time, each pointing at something on the page. */
export function Tour({ steps, value: step, onChange: onStepChange, onFinish, nextLabel = liuliText('@liuli.next'), backLabel = liuliText('@liuli.back'), skipLabel = liuliText('@liuli.skip'), doneLabel = liuliText('@liuli.done'), stepLabel = (step: number, count: number) => liuliText('@liuli.stepOf', { step: step + 1, count }), 'data-testid': testId, ...attributes }: TourProps) {
  const id = useId();
  const at = step === null ? null : Math.min(Math.max(0, step), steps.length - 1);
  const current = at === null ? undefined : steps[at];
  const placement = current?.placement ?? 'bottom';
  // The target is brought into view before its callout is shown beside it.
  const go = (next: number | null) => { if (next !== null) reveal(steps[next].target); onStepChange(next); };
  const isLast = at !== null && at === steps.length - 1;
  return (
    <view
      id={id}
      popover="manual"
      anchor={current?.target ?? steps[0]?.target}
      open={current !== undefined}
      onToggle={isOpen => { if (!isOpen) onStepChange(null); }}
      role="group"
      aria-label={current === undefined ? undefined : `${current.targetLabel}: ${current.title}`}
      data-testid={testId} {...dataOf(attributes)}
      style={{ positionArea: areas[placement] }}
      className={r.callout.base}
    >
      {/* One caret per edge, on the target's centre line; each shows only on the side the callout ended on. */}
      {sides.map(side => <view key={side} aria-hidden className={cx(r.caret.base, r.caret.side[side])} data-testid={testId && `${testId}-caret-${side}`} />)}
      {current !== undefined && at !== null && (
        <>
          <text role="heading" headingLevel={2} className={r.title.base}>{current.title}</text>
          <text className={r.text.base}>{current.text}</text>
          <view className={r.footer.base}>
            <text className={r.count.base}>{stepLabel(at, steps.length)}</text>
            <view className={r.actions.base}>
              {!isLast && <Button label={skipLabel} variant="ghost" size="sm" onClick={() => go(null)} className={r.quiet.base} data-testid={testId && `${testId}-skip`} />}
              {at > 0 && <Button label={backLabel} variant="ghost" size="sm" onClick={() => go(at - 1)} className={r.quiet.base} data-testid={testId && `${testId}-back`} />}
              <Button
                label={isLast ? doneLabel : nextLabel}
                size="sm"
                onClick={() => { if (isLast) { onFinish?.(); go(null); } else go(at + 1); }}
                data-testid={testId && `${testId}-next`}
              />
            </view>
          </view>
        </>
      )}
    </view>
  );
}
