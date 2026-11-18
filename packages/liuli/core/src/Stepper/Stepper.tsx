import { cx } from '@lingxia/view';
import { Glyph } from '../internal/Glyph';
import { stepperRecipe as r } from './Stepper.recipe';

export type StepperOrientation = 'horizontal' | 'vertical';
type StepState = 'done' | 'current' | 'upcoming';

export interface StepperStep {
  /** Identifies the step: its test id is `<id>-<key>`. */
  key: string;
  label: string;
  /** A second line; vertical only. */
  description?: string;
}

export interface StepperProps {
  /** Names the steps for assistive tech: "Checkout steps". */
  label: string;
  /** The steps, in order. */
  steps: readonly StepperStep[];
  /** The index of the step in progress, from 0; `steps.length` when every step is done. */
  value: number;
  /** What a screen reader hears for a step's state: `{ done: 'Done', current: 'Current step', upcoming: 'Not started' }`. */
  stateLabels: Record<StepState, string>;
  /** Makes a finished step a button that goes back to it; runs with its index. */
  onChange?: (index: number) => void;
  /** `horizontal` suits up to four short labels; `vertical` any number, with descriptions. @default 'vertical' */
  orientation?: StepperOrientation;
  className?: string;
  'data-testid'?: string;
}

/** Where someone is in a task with a known number of steps. */
export function Stepper({ label, steps, value: current, stateLabels, onChange: onStepClick, orientation = 'vertical', className, 'data-testid': testId }: StepperProps) {
  const isRow = orientation === 'horizontal';
  return (
    <view role="list" aria-label={label} data-testid={testId} className={cx(r.root.base, r.root.orientation[orientation], className)}>
      {steps.map((step, index) => {
        const state: StepState = index < current ? 'done' : index === current ? 'current' : 'upcoming';
        const isLast = index === steps.length - 1;
        const marker = (
          <view aria-hidden className={cx(r.marker.base, r.marker.state[state])}>
            {state === 'done'
              ? <Glyph name="check" weight={2.5} className={r.marker.tick} />
              : <text className={cx(r.number.base, r.number.state[state])}>{String(index + 1)}</text>}
          </view>
        );
        // The line to the next step is filled once this step is done.
        const line = !isLast && <view aria-hidden className={cx(r.line.base, r.line.orientation[orientation], state === 'done' ? r.line.done : r.line.ahead)} />;
        const text = (
          <view className={cx(r.text.base, r.text.orientation[orientation])}>
            <text className={cx(r.label.base, r.label.state[state], isRow && r.label.row)}>{step.label}</text>
            {!isRow && step.description !== undefined && <text className={r.description.base}>{step.description}</text>}
          </view>
        );
        const body = isRow
          ? [<view key="rail" className={r.rail.row}>{marker}{line}</view>, <view key="text">{text}</view>]
          : [<view key="rail" className={r.rail.column}>{marker}{line}</view>, <view key="text" className={cx(r.body.column, !isLast && r.body.gap)}>{text}</view>];
        const name = `${step.label}, ${stateLabels[state]}`;
        const shape = cx(r.step.base, r.step.orientation[orientation]);
        return (
          <view key={step.key} role="listitem" aria-label={state === 'done' && onStepClick ? undefined : name} className={cx(isRow && !isLast && r.item.grow, isRow && r.item.row)}>
            {state === 'done' && onStepClick
              ? <button aria-label={name} onClick={() => onStepClick(index)} data-testid={testId && `${testId}-${step.key}`} className={cx(shape, r.step.pressable)}>{body}</button>
              : <view data-testid={testId && `${testId}-${step.key}`} className={shape}>{body}</view>}
          </view>
        );
      })}
    </view>
  );
}
