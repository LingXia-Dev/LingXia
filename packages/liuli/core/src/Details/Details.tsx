import { createContext, cx, useContext, useLocal, type Child, type Local } from '@lingxia/view';
import type { IconRef } from '../Icon/Icon';
import { Glyph } from '../internal/Glyph';
import { LayerContext } from '../internal/layer';
import { detailsRecipe as r } from './Details.recipe';
import { dataOf } from '../internal/data';

interface GroupState {
  /** Set in an exclusive group: the open row's value, or null. */
  open: string | null;
  isExclusive: boolean;
  set: (value: string | null) => void;
}

const Group = createContext<GroupState | null>(null);

export interface DetailsProps {
  /** The summary row's text; also what the toggle is named. */
  label: string;
  /** A second, quieter line under the label: what is inside. */
  description?: string;
  /** An icon before the label, by name or as a definition; not read. */
  startIcon?: IconRef;
  /** The body, which exists only while open. */
  children: Child;
  /** Identifies the row in an exclusive `DetailsGroup`. */
  value?: string;
  /** Open on first render, uncontrolled. @default false */
  isDefaultOpen?: boolean;
  /** Controlled open state; pass `onOpenChange` with it. */
  isOpen?: boolean;
  onOpenChange?: (isOpen: boolean) => void;
  /** A View-local cell the row reads and writes its open state in, instead of `isOpen` with `onOpenChange`. */
  bind?: Local<boolean>;
  /** Cannot be opened or closed. @default false */
  isDisabled?: boolean;
  className?: string;
  /** On the disclosure. */
  'data-testid'?: string;
}

/** A summary row that shows and hides the content under it. */
export function Details({ label, description, startIcon, children, value, isDefaultOpen = false, isOpen, onOpenChange, bind, isDisabled = false, className, 'data-testid': testId, ...attributes }: DetailsProps) {
  const group = useContext(Group);
  const local = useLocal(isDefaultOpen);
  const exclusive = group?.isExclusive && value !== undefined;
  const open = exclusive ? group.open === value : bind ? bind.value : isOpen ?? local.value;
  const toggle = (next: boolean) => {
    if (exclusive) group.set(next ? value : group.open === value ? null : group.open);
    else if (bind) bind.set(next);
    else if (isOpen === undefined) local.set(next);
    onOpenChange?.(next);
  };
  const inGroup = group !== null;
  return (
    // A button with aria-expanded, as the Details primitive builds, so the chevron can sit at the end.
    <view data-testid={testId} {...dataOf(attributes)} className={cx(r.root.base, className)}>
      <button
        aria-expanded={open}
        data-state={open ? 'open' : 'closed'}
        disabled={isDisabled}
        onClick={() => toggle(!open)}
        data-testid={testId && `${testId}-summary`}
        className={cx(r.summary.base, inGroup ? r.summary.inGroup : r.summary.alone)}
      >
        {startIcon !== undefined && <Glyph name={startIcon} className={r.icon.base} />}
        <view className={r.text.base}>
          <text className={r.label.base}>{label}</text>
          {description !== undefined && <text className={r.description.base}>{description}</text>}
        </view>
        <Glyph name="chevron-down" className={cx(r.chevron.base, open && r.chevron.open)} />
      </button>
      {/* Kept in the tree so the box can close around it; closed, it is clipped to nothing and hidden, which takes it out of reach of focus, presses and assistive tech. */}
      <view aria-hidden={!open || undefined} data-testid={testId && `${testId}-body`} className={cx(r.box.base, !open && r.box.closed)}>
        <view className={cx(r.body.base, inGroup && r.body.inGroup)}>{children}</view>
      </view>
    </view>
  );
}

export interface DetailsGroupProps {
  /** Details rows. */
  children: Child;
  /** Opening one row closes the others; each row needs a `value`. @default false */
  isExclusive?: boolean;
  /** In an exclusive group, the row open at first. */
  defaultValue?: string;
  /** In an exclusive group, the open row, controlled: its value, or `null` for none. Pass `onChange` with it. */
  value?: string | null;
  onChange?: (value: string | null) => void;
  /** In an exclusive group, a View-local cell holding the open row's value, or `null`; instead of `value` with `onChange`. */
  bind?: Local<string | null>;
  /** Names the group for assistive tech: "Questions". */
  label?: string;
  className?: string;
  'data-testid'?: string;
}

/** Details rows in one bordered list: an accordion. */
export function DetailsGroup({ children, isExclusive = false, defaultValue, value, onChange, bind, label, className, 'data-testid': testId, ...attributes }: DetailsGroupProps) {
  const own = useLocal<string | null>(defaultValue ?? null);
  const current = bind ? bind.value : value === undefined ? own.value : value;
  const set = (next: string | null) => {
    if (bind) bind.set(next);
    else if (value === undefined) own.set(next);
    onChange?.(next);
  };
  const layer = useContext(LayerContext);
  return (
    <Group.Provider value={{ open: current, isExclusive, set }}>
      <view role={label === undefined ? undefined : 'group'} aria-label={label} data-testid={testId} {...dataOf(attributes)} className={cx(r.group.base, r.group.layer[layer], className)}>{children}</view>
    </Group.Provider>
  );
}
