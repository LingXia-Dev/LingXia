import { useLocal, type Child } from '@lingxia/view';

// For examples and the gallery only: an app has its own answers to a press. Every specimen that
// can be pressed answers visibly, so nothing on a page looks pressable and then does nothing.

/**
 * A specimen and, under it, a quiet line saying what its last press did: "Opened Coast, November".
 * For presses whose result the component does not show itself: a row with a chevron, a button, a
 * menu item. Short, past tense, the thing's own name. Before the first press there is no line:
 * a specimen at rest shows only itself. The line then stays and follows each press.
 */
export function Said({ children }: { children: (say: (text: string) => void) => Child }) {
  const last = useLocal('');
  return (
    // The specimen grows to take its first line and does not snap.
    <view className="gap-2 motion-safe:transition-size duration-base ease-standard">
      {children(last.set)}
      {last.value !== '' && <text role="status" className="text-footnote text-muted">{last.value}</text>}
    </view>
  );
}

/**
 * Holds one value for a specimen that shows its own state: a switch that flips, a field that
 * takes text, a tab that changes. Each use keeps its own value.
 */
export function Held<T extends Value>({ value, children }: { value: T; children: (value: Widen<T>, set: (next: Widen<T>) => void) => Child }) {
  const held = useLocal<Value>(value);
  return <>{children(held.value as Widen<T>, next => held.set(next as Value))}</>;
}

type Value = string | number | boolean | null | readonly string[];
// `true` holds a boolean and `'week'` a string: the value's kind, not the first value.
type Widen<T> = T extends boolean ? boolean : T extends string ? string : T extends number ? number : T;

/**
 * The handler of a control that cannot be used as drawn: disabled, read-only, busy, or a still
 * picture on a tile, which takes the press itself. Never for a control that answers a press.
 */
export const unused = (): void => {};
