// The h-app primitives the ports use (components/ui/button, badge, skeleton, network/status), with
// their class strings kept. A caller's class overrides the variant's, as with h-app's twMerge `cn`.
import type { VectorPath } from '@lingxia/ui';
import { icons, type IconName } from './icons';

export const cn = (...parts: (string | false | null | undefined)[]) => parts.filter(Boolean).join(' ');

export function Icon({ name, size = 24, strokeWidth = 2, fill = 'none', className }: { name: IconName; size?: number; strokeWidth?: number; fill?: string; className?: string }) {
  const paths: VectorPath[] = icons[name].map(d => ({ d, fill, stroke: 'currentColor', strokeWidth, strokeLinecap: 'round', strokeLinejoin: 'round' }));
  return <vector accessibilityHidden paths={paths} className={cn('shrink-0', className)} style={{ width: size, height: size }} />;
}

const BUTTON = 'focus-ring inline-flex shrink-0 items-center justify-center gap-2 whitespace-nowrap rounded-md transition duration-[var(--duration-fast)] active:opacity-90 motion-reduce:transition-none';
const VARIANT = {
  default: 'bg-primary text-primary-foreground',
  secondary: 'bg-secondary text-secondary-foreground',
  outline: 'border border-input bg-field',
  ghost: 'bg-transparent',
  destructive: 'bg-destructive text-destructive-foreground',
};
const SIZE = {
  default: 'h-control px-4 text-control',
  sm: 'h-control-small rounded-sm px-3 text-control-small',
  lg: 'h-control-large px-6 text-control',
  icon: 'size-control text-control',
};

export function Button({ variant = 'default', size = 'default', className, disabled, onClick, accessibilityLabel, testId, children }: {
  variant?: keyof typeof VARIANT; size?: keyof typeof SIZE; className?: string; disabled?: boolean; onClick?: () => void;
  accessibilityLabel?: string; testId?: string; children?: JSX.Element | string | (JSX.Element | string | null)[];
}) {
  const inert = disabled && (variant === 'default' || variant === 'destructive') ? 'bg-surface-sunken text-muted-foreground' : disabled ? 'opacity-50' : '';
  return (
    <button data-testid={testId} accessibilityLabel={accessibilityLabel} disabled={disabled} onClick={onClick}
      className={cn(BUTTON, inert || VARIANT[variant], SIZE[size], className)}>
      {children}
    </button>
  );
}

const BADGE = {
  default: 'bg-primary text-primary-foreground',
  secondary: 'bg-secondary text-secondary-foreground',
  success: 'bg-success-subtle text-success-subtle-foreground',
  warning: 'bg-warning-subtle text-warning-subtle-foreground',
  destructive: 'bg-destructive-subtle text-destructive-subtle-foreground',
};

export function Badge({ variant = 'default', className, children }: { variant?: keyof typeof BADGE; className?: string; children: (JSX.Element | string)[] | string }) {
  return <row className={cn('min-h-6 items-center rounded-full px-2.5 py-0.5 text-caption font-semibold', BADGE[variant], className)}>{children}</row>;
}

export function Skeleton({ className }: { className?: string }) {
  return <view accessibilityHidden className={cn('animate-pulse rounded-md bg-surface-sunken', className)} />;
}

export type StatusKind = 'online' | 'degraded' | 'offline' | 'unknown';

export function StatusDot({ status, size = 10 }: { status: StatusKind; size?: number }) {
  return (
    <view accessibilityLabel={status} style={{ width: size, height: size }} className={cn(
      'rounded-full',
      status === 'online' && 'bg-success',
      status === 'degraded' && 'bg-warning',
      status === 'offline' && 'bg-muted-foreground opacity-50',
      status === 'unknown' && 'border border-muted-foreground bg-transparent opacity-60',
    )} />
  );
}
