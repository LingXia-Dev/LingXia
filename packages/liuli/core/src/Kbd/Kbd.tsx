import { cx } from '@lingxia/view';
import { kbdRecipe as r } from './Kbd.recipe';
import { dataOf } from '../internal/data';

export type KbdSize = 'sm' | 'md';

export interface KbdProps {
  /** The keys of one shortcut, in the order they are held: `['⌘', 'K']`. Each is drawn as a key cap. */
  keys: readonly string[];
  /** How the shortcut is read: "Command K". Without it the keys are read as written. */
  label?: string;
  /** @default 'sm' */
  size?: KbdSize;
  className?: string;
  'data-testid'?: string;
}

/** A keyboard shortcut, drawn as key caps. */
export function Kbd({ keys, label, size = 'sm', className, 'data-testid': testId, ...attributes }: KbdProps) {
  return (
    <view data-testid={testId} {...dataOf(attributes)} role={label === undefined ? undefined : 'img'} aria-label={label} className={cx(r.root.base, className)}>
      {keys.map((key, at) => (
        <view key={`${at}-${key}`} aria-hidden={label !== undefined} className={cx(r.key.base, r.key.size[size])}>
          <text className={cx(r.label.base, r.label.size[size])}>{key}</text>
        </view>
      ))}
    </view>
  );
}
