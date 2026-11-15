import { cx } from '@lingxia/view';
import { timerRecipe as r } from './Timer.recipe';

export type TimerSize = 'sm' | 'md' | 'lg';

export interface TimerProps {
  /** The time to show, in seconds: what is left of a countdown, or what has passed. The app counts; Timer draws. */
  seconds: number;
  /** How it is read: "2 minutes 5 seconds left". Without it the digits are read as written. */
  'aria-label'?: string;
  /** Always shows hours, as `0:02:05`. Otherwise hours appear from one hour on. @default false */
  hasHours?: boolean;
  /** @default 'md' */
  size?: TimerSize;
  className?: string;
  'data-testid'?: string;
}

const two = (n: number) => String(n).padStart(2, '0');

/** `m:ss`, or `h:mm:ss` from an hour on. */
export function formatTimer(seconds: number, hasHours = false): string {
  const whole = Math.max(0, Math.floor(seconds));
  const [h, m, s] = [Math.floor(whole / 3600), Math.floor((whole % 3600) / 60), whole % 60];
  return h > 0 || hasHours ? `${h}:${two(m)}:${two(s)}` : `${m}:${two(s)}`;
}

/** A length of time in steady digits: a countdown, a recording, a workout. */
export function Timer({ seconds, 'aria-label': ariaLabel, hasHours = false, size = 'md', className, 'data-testid': testId }: TimerProps) {
  return (
    <text data-testid={testId} aria-label={ariaLabel} className={cx(r.root.base, r.root.size[size], seconds <= 0 && r.root.done, className)}>
      {formatTimer(seconds, hasHours)}
    </text>
  );
}
