import { cx, type Child } from '@lingxia/view';
import { ButtonGroupContext } from '../Button/Button';
import { useControlSize } from '../density';
import type { FieldSize } from '../Field/Field';
import { dataOf } from '../internal/data';
import { JoinedContext, joinedCorners, joinedPlace, useJoined, type JoinedEdge } from '../internal/joined';
import { inputGroupRecipe as r } from './InputGroup.recipe';

export interface InputGroupProps {
  /** What the joined controls enter together, read once as the group's name: "Website", "Price". Each control keeps its own name. */
  label: string;
  /** TextInput, Select, Button, IconButton and InputGroupText elements, in reading order. */
  children: Child;
  /** The size of every control that sets none. By default `md`, or `sm` where the screen is compact. */
  size?: FieldSize;
  className?: string;
  'data-testid'?: string;
}

/** Flattens `children` into its elements, through arrays, leaving out empty slots. */
function elements(children: unknown): unknown[] {
  if (Array.isArray(children)) return children.flatMap(elements);
  return children === null || children === undefined || typeof children === 'boolean' ? [] : [children];
}

const edgeAt = (index: number, count: number): JoinedEdge => (count === 1 ? 'only' : index === 0 ? 'start' : index === count - 1 ? 'end' : 'middle');

/** Controls that enter one value together, joined edge to edge: a field with a unit, a scheme, a currency or an action. */
export function InputGroup({ label, children, size: ownSize, className, 'data-testid': testId, ...attributes }: InputGroupProps) {
  const size = useControlSize(ownSize);
  const items = elements(children);
  return (
    <ButtonGroupContext.Provider value={{ isAttached: true, size, variant: 'secondary', isDisabled: false, isFieldEdge: true }}>
      <view role="group" aria-label={label} data-testid={testId} {...dataOf(attributes)} className={cx(r.root.base, className)}>
        {items.map((item, index) => <JoinedContext.Provider key={String(index)} value={{ edge: edgeAt(index, items.length), size }}>{item as JSX.Element}</JoinedContext.Provider>)}
      </view>
    </ButtonGroupContext.Provider>
  );
}

export interface InputGroupTextProps {
  /** The fixed text: "https://", "@example.com", "kg". It is read with the field beside it. */
  children: string;
  size?: FieldSize;
  className?: string;
  'data-testid'?: string;
}

/** Fixed text joined to a field in an InputGroup. */
export function InputGroupText({ children, size: ownSize, className, 'data-testid': testId }: InputGroupTextProps) {
  const group = useJoined();
  const size = useControlSize(ownSize ?? group?.size);
  const edge = group?.edge ?? 'only';
  return (
    <view data-testid={testId} className={cx(r.text.base, r.text.size[size], edge === 'only' ? r.text.alone : joinedCorners[edge], joinedPlace[edge], className)}>
      <text className={cx(r.textLabel.base, r.textLabel.size[size])}>{children}</text>
    </view>
  );
}
