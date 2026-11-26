import { cx, type Local } from '@lingxia/view';
import { liuliText } from '../i18n/text';
import { Glyph } from '../internal/Glyph';
import { stepperRecipe as r } from './Stepper.recipe';
import { dataOf } from '../internal/data';

export type StepperOrientation = 'horizontal' | 'vertical';
type StepState = 'done' | 'current' | 'upcoming' | 'error';

export interface StepperStep {
  /** Identifies the step: its test id is `<id>-<key>`. */
  key: string;
  label: string;
  /** A second line; vertical only. */
  description?: string;
  /** Marks a step with a problem to fix, such as a rejected payment: a danger mark in place of its number or tick, read as needing attention. */
  status?: 'error';
  /** A finished step that cannot be gone back to: it stays a plain step, not a button. @default false */
  isDisabled?: boolean;
}

export type StepperBinding =
  | {
      /** A View-local cell the control reads and writes; use it or `value` with `onChange`. */
      bind: Local<number>;
      value?: never;
      onChange?: never;
    }
  | {
      bind?: never;
      /** The index of the step in progress, from 0; `steps.length` when every step is done. */
      value: number;
      /** Makes a finished step a button that goes back to it; runs with its index. */
      onChange?: (index: number) => void;
    };

export type StepperProps = StepperBinding & {
  /** Names the steps for assistive tech: "Checkout steps". */
  label: string;
  /** The steps, in order. */
  steps: readonly StepperStep[];
  /** What a screen reader hears for a step's state, instead of Liuli's "Done", "Current step", "Not started" and "Needs attention" in the page's language. */
  stateLabels?: Partial<Record<StepState, string>>;
  /** `horizontal` suits up to four short labels; `vertical` any number, with descriptions. @default 'vertical' */
  orientation?: StepperOrientation;
  className?: string;
  'data-testid'?: string;
};

/** Where someone is in a task with a known number of steps. */
export function Stepper({ label, steps, bind, value: own, stateLabels, onChange: ownChange, orientation = 'vertical', className, 'data-testid': testId, ...attributes }: StepperProps) {
  const current = bind ? bind.value : own;
  // Bound, a finished step is a button that goes back to it.
  const onStepClick = bind ? (index: number) => bind.set(index) : ownChange;
  const isRow = orientation === 'horizontal';
  return (
    <view role="list" aria-label={label} data-testid={testId} {...dataOf(attributes)} className={cx(r.root.base, r.root.orientation[orientation], className)}>
      {steps.map((step, index) => {
        const progress = index < current ? 'done' : index === current ? 'current' : 'upcoming';
        const state: StepState = step.status === 'error' ? 'error' : progress;
        const canGoBack = progress === 'done' && !step.isDisabled && onStepClick !== undefined;
        const isLast = index === steps.length - 1;
        const marker = (
          <view aria-hidden className={cx(r.marker.base, r.marker.state[state])}>
            {state === 'error'
              ? <Glyph name="error" className={r.marker.tick} />
              : state === 'done'
                ? <Glyph name="check" weight={2.5} className={r.marker.tick} />
                : <text className={cx(r.number.base, r.number.state[state])}>{String(index + 1)}</text>}
          </view>
        );
        // The line to the next step is filled once this step is done.
        const line = !isLast && <view aria-hidden className={cx(r.line.base, r.line.orientation[orientation], progress === 'done' ? r.line.done : r.line.ahead)} />;
        const text = (
          <view className={cx(r.text.base, r.text.orientation[orientation])}>
            <text className={cx(r.label.base, r.label.state[state], isRow && r.label.row)}>{step.label}</text>
            {!isRow && step.description !== undefined && <text className={r.description.base}>{step.description}</text>}
          </view>
        );
        const body = isRow
          ? [<view key="rail" className={r.rail.row}>{marker}{line}</view>, <view key="text">{text}</view>]
          : [<view key="rail" className={r.rail.column}>{marker}{line}</view>, <view key="text" className={cx(r.body.column, !isLast && r.body.gap)}>{text}</view>];
        const name = `${step.label}, ${stateLabels?.[state] ?? liuliText(`@liuli.step.${state}`)}`;
        const shape = cx(r.step.base, r.step.orientation[orientation]);
        return (
          <view key={step.key} role="listitem" aria-label={canGoBack ? undefined : name} className={cx(isRow && !isLast && r.item.grow, isRow && r.item.row)}>
            {canGoBack
              ? <button aria-label={name} onClick={() => onStepClick(index)} data-testid={testId && `${testId}-${step.key}`} className={cx(shape, r.step.pressable)}>{body}</button>
              : <view data-testid={testId && `${testId}-${step.key}`} className={shape}>{body}</view>}
          </view>
        );
      })}
    </view>
  );
}
