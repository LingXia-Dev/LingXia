// Pages with dots and progress, drawn from View primitives for the lab's specimens.
import { childrenToArray, cx, type Child, type Local } from '@lingxia/view';

// Pages side by side that stop on each one; a screen reader reads them as a named list ("2 of 3")
// and the dots, out of its tree, follow the page and jump on tap.
export function Pager({ bind, label, testId, className, children }: { bind: Local<number>; label: string; testId: string; className?: string; children?: Child }) {
  const pages = childrenToArray(children);
  const at = Math.min(Math.max(0, Math.round(bind.value)), Math.max(0, pages.length - 1));
  return (
    <view className={cx('gap-2', className)} data-testid={testId}>
      <scroll bind={bind} data-testid={`${testId}-scroll`} className="overflow-x-auto snap-x snap-always">
        <view className="flex-row" role="list" aria-label={label}>
          {pages.map((content, index) => (
            <view key={String(index)} role="listitem" data-testid={`${testId}-page-${index}`} className="w-full shrink-0 snap-start">{content}</view>
          ))}
        </view>
      </scroll>
      {pages.length > 1 && (
        <view aria-hidden className="flex-row justify-center items-center" data-testid={`${testId}-dots`}>
          {/* A finger-sized target around each small mark. */}
          {pages.map((_, index) => (
            <button key={String(index)} data-testid={`${testId}-dot-${index}`} aria-label={`${index + 1}`} onClick={() => bind.set(index)} className="py-[9px] px-[3px] bg-transparent">
              <view data-testid={`${testId}-mark-${index}`}
                className={cx('h-1.5 rounded-full transition-all duration-200', index === at ? 'w-[18px] bg-accent' : 'w-1.5 bg-line')} />
            </button>
          ))}
        </view>
      )}
    </view>
  );
}

const share = (value: number) => Math.min(1, Math.max(0, value / 100));

/** A bar of `value` out of 100; without one it pulses, indeterminate. */
export function ProgressBar({ value, label, testId }: { value?: number; label: string; testId: string }) {
  return (
    <view role="progressbar" aria-label={label} data-testid={testId} className="h-1 rounded-full bg-surface-sunken overflow-hidden"
      {...(value === undefined ? {} : { 'aria-valuenow': Math.min(100, Math.max(0, value)) })}>
      {value === undefined
        ? <view className="w-2/5 h-full rounded-full bg-accent animate-pulse" />
        : <view data-testid={`${testId}-fill`} className="h-full rounded-full bg-accent" style={{ width: `${share(value) * 100}%` }} />}
    </view>
  );
}

const circle = 'M12 2a10 10 0 1 1 0 20a10 10 0 1 1 0-20';
const around = 2 * Math.PI * 10;

/** A ring of `value` out of 100; without one a quarter arc spins, indeterminate. */
export function ProgressRing({ value, label, valueText, testId, small }: { value?: number; label: string; valueText?: string; testId: string; small?: boolean }) {
  const arc = value === undefined ? around / 4 : around * share(value);
  return (
    <view role="progressbar" aria-label={label} data-testid={testId} className={small ? 'size-6' : 'size-10'}
      {...(value === undefined ? {} : { 'aria-valuenow': Math.min(100, Math.max(0, value)), 'aria-valuetext': valueText })}>
      <vector aria-hidden className={cx(small ? 'size-6' : 'size-10', value === undefined && 'animate-spin')} paths={[
        { d: circle, fill: 'none', stroke: 'line', strokeWidth: 2.5 },
        { d: circle, fill: 'none', stroke: 'accent', strokeWidth: 2.5, strokeLinecap: 'round', strokeDasharray: [arc, around] },
      ]} />
    </view>
  );
}
