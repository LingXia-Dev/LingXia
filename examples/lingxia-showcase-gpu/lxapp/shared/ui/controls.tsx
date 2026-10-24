import type { Child, Local } from '@lingxia/ui';

export type ButtonVariant = 'primary' | 'secondary' | 'destructive' | 'ghost';
const variants: Record<ButtonVariant, string> = {
  primary: 'bg-primary text-primary-foreground active:opacity-80',
  secondary: 'bg-secondary text-secondary-foreground active:bg-surface-200',
  destructive: 'bg-destructive text-destructive-foreground active:opacity-80',
  ghost: 'bg-transparent text-primary active:bg-muted',
};
const sizes = { md: 'min-h-11 px-4 py-2.5 rounded-xl text-sm', sm: 'min-h-9 px-3 py-1.5 rounded-lg text-xs' };

export function Button({
  variant = 'primary',
  size = 'md',
  testId,
  label,
  disabled,
  onClick,
  className = '',
  children,
}: {
  variant?: ButtonVariant;
  size?: keyof typeof sizes;
  testId?: string;
  label?: string;
  disabled?: boolean;
  onClick?: () => unknown;
  className?: string;
  children?: Child;
}) {
  return (
    <button
      testId={testId}
      accessibilityLabel={label}
      disabled={disabled}
      onClick={
        onClick
          ? () => {
              void onClick();
            }
          : undefined
      }
      className={`${sizes[size]} ${variants[variant]} font-medium text-center items-center justify-center disabled:opacity-40 ${className}`}
    >
      {children}
    </button>
  );
}

/** A labeled single-line editor bound to a local string. */
export function Field({
  label,
  testId,
  bind,
  placeholder,
  keyboard,
  secure,
  disabled,
  onSubmit,
}: {
  label: string;
  testId?: string;
  bind: Local<string>;
  placeholder?: string;
  keyboard?: 'text' | 'number' | 'email' | 'phone';
  secure?: boolean;
  disabled?: boolean;
  onSubmit?: (value: string) => void;
}) {
  return (
    <column className="gap-1.5">
      <text className="text-sm font-medium">{label}</text>
      <input
        testId={testId}
        accessibilityLabel={label}
        bind={bind}
        placeholder={placeholder}
        keyboard={keyboard}
        secure={secure}
        disabled={disabled}
        onSubmit={onSubmit}
        className="min-h-11 px-3 py-2 rounded-lg border border-input bg-card text-base placeholder:text-muted-foreground focus:border-ring"
      />
    </column>
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
    <row testId={testId} className="gap-1 p-1 bg-muted rounded-xl">
      {options.map(option => (
        <button
          key={option.value}
          testId={`${testId}-${option.value}`}
          disabled={disabled}
          onClick={() => {
            void onChange(option.value);
          }}
          className={
            option.value === value
              ? 'grow basis-0 min-h-8 px-2 py-1 rounded-lg text-xs font-medium text-center bg-card shadow-sm text-foreground'
              : 'grow basis-0 min-h-8 px-2 py-1 rounded-lg text-xs font-medium text-center bg-transparent text-muted-foreground'
          }
        >
          {option.label}
        </button>
      ))}
    </row>
  );
}
