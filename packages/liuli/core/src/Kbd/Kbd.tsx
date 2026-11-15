import { cx } from '@lingxia/view';
import { kbdRecipe as r } from './Kbd.recipe';

export type KbdSize = 'sm' | 'md';

export interface KbdProps {
  /** The keys of one shortcut, in the order they are held: `['Ctrl', 'K']`. Each is drawn as a key cap. */
  keys: readonly string[];
  /** How the shortcut is read: "Control K". Without it the keys are read as written. */
  'aria-label'?: string;
  /** @default 'sm' */
  size?: KbdSize;
  className?: string;
  'data-testid'?: string;
}

/** A keyboard shortcut, drawn as key caps. */
export function Kbd({ keys, 'aria-label': ariaLabel, size = 'sm', className, 'data-testid': testId }: KbdProps) {
  return (
    <view data-testid={testId} role={ariaLabel === undefined ? undefined : 'img'} aria-label={ariaLabel} className={cx(r.root.base, className)}>
      {keys.map((key, at) => (
        <view key={`${at}-${key}`} aria-hidden={ariaLabel !== undefined} className={cx(r.key.base, r.key.size[size])}>
          <text className={cx(r.label.base, r.label.size[size])}>{key}</text>
        </view>
      ))}
    </view>
  );
}
