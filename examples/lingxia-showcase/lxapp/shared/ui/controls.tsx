import type { Child, LinkTarget, Local } from '@lingxia/view';
import type { DataAttributes } from './layout';

export type ButtonVariant = 'primary' | 'secondary' | 'destructive' | 'ghost';
const variants: Record<ButtonVariant, string> = {
  primary: 'bg-accent-solid text-on-accent-solid hover:bg-accent-solid-hover active:bg-accent-solid-pressed',
  secondary: 'bg-surface-sunken text-text active:bg-line',
  destructive: 'bg-danger-solid text-on-danger-solid hover:bg-danger-solid-hover active:bg-danger-solid-pressed',
  ghost: 'bg-transparent text-accent active:bg-surface-sunken',
};
const sizes = { md: 'min-h-11 px-4 py-2.5 rounded-xl text-sm', sm: 'min-h-9 px-3 py-1.5 rounded-lg text-xs' };

export function Button({
  variant = 'primary',
  size = 'md',
  testId,
  data,
  label,
  disabled,
  onClick,
  to,
  className = '',
  children,
}: {
  variant?: ButtonVariant;
  size?: keyof typeof sizes;
  testId?: string;
  data?: DataAttributes;
  label?: string;
  disabled?: boolean;
  onClick?: () => unknown;
  /** Navigates without an action: the button's look, a link's semantics. */
  to?: LinkTarget;
  className?: string;
  children?: Child;
}) {
  const look = `${sizes[size]} ${variants[variant]} font-medium text-center items-center-safe justify-center-safe disabled:opacity-40 ${className}`;
  if (to) {
    return (
      <link data-testid={testId} {...data} aria-label={label} disabled={disabled} to={to} className={look}>
        {children}
      </link>
    );
  }
  return (
    <button
      data-testid={testId}
      {...data}
      aria-label={label}
      disabled={disabled}
      onClick={
        onClick
          ? () => {
              void onClick();
            }
          : undefined
      }
      className={look}
    >
      {children}
    </button>
  );
}

/** A labeled single-line editor bound to a local string. */
export function Field({
  label,
  testId,
  data,
  bind,
  placeholder,
  type,
  disabled,
  onSubmit,
}: {
  label: string;
  testId?: string;
  data?: DataAttributes;
  bind: Local<string>;
  placeholder?: string;
  type?: 'text' | 'password' | 'number' | 'email' | 'tel';
  disabled?: boolean;
  onSubmit?: (value: string) => void;
}) {
  return (
    <view className="gap-1.5">
      <text className="text-sm font-medium">{label}</text>
      <input
        data-testid={testId}
        {...data}
        aria-label={label}
        bind={bind}
        placeholder={placeholder}
        type={type}
        disabled={disabled}
        onSubmit={onSubmit}
        className="min-h-11 px-3 py-2 rounded-lg border border-line-strong bg-surface text-base placeholder:text-muted focus:border-focus"
      />
    </view>
  );
}

/** Exclusive choices rendered as one segmented row. */
export function Segmented<T extends string>({
  testId,
  value,
  options,
  onChange,
  disabled,
}: {
  testId: string;
  value: T;
  options: readonly { value: T; label: string }[];
  onChange: (value: T) => unknown;
  disabled?: boolean;
}) {
  return (
    <view data-testid={testId} className="flex-row gap-1 p-1 bg-surface-sunken rounded-xl">
      {options.map(option => (
        <button
          key={option.value}
          data-testid={`${testId}-${option.value}`}
          data-selected={String(option.value === value)}
          disabled={disabled}
          onClick={() => {
            void onChange(option.value);
          }}
          className={
            option.value === value
              ? 'grow basis-0 min-h-8 px-2 py-1 rounded-lg text-xs font-medium text-center bg-surface shadow-sm text-text'
              : 'grow basis-0 min-h-8 px-2 py-1 rounded-lg text-xs font-medium text-center bg-transparent text-muted'
          }
        >
          {option.label}
        </button>
      ))}
    </view>
  );
}
