import { createContext, cx, useContext, useLocal, type Child } from '@lingxia/view';
import { glyphBox, glyphs } from '../internal/glyphs';
import { LayerContext } from '../internal/layer';
import { detailsRecipe as r } from './Details.recipe';

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
  /** The body, which exists only while open. */
  children: Child;
  /** Identifies the row in an exclusive `DetailsGroup`. */
  value?: string;
  /** Open on first render, uncontrolled. @default false */
  isDefaultOpen?: boolean;
  /** Controlled open state; pass `onOpenChange` with it. */
  isOpen?: boolean;
  onOpenChange?: (isOpen: boolean) => void;
  className?: string;
  /** On the disclosure. */
  'data-testid'?: string;
}

/** A summary row that shows and hides the content under it. */
export function Details({ label, children, value, isDefaultOpen = false, isOpen, onOpenChange, className, 'data-testid': testId }: DetailsProps) {
  const group = useContext(Group);
  const local = useLocal(isDefaultOpen);
  const exclusive = group?.isExclusive && value !== undefined;
  const open = exclusive ? group.open === value : isOpen ?? local.value;
  const toggle = (next: boolean) => {
    if (exclusive) group.set(next ? value : group.open === value ? null : group.open);
    else if (isOpen === undefined) local.set(next);
    onOpenChange?.(next);
  };
  const inGroup = group !== null;
  return (
    // A button with aria-expanded, as the Details primitive builds, so the chevron can sit at the end.
    <view data-testid={testId} className={cx(r.root.base, className)}>
      <button
        aria-expanded={open}
        data-state={open ? 'open' : 'closed'}
        onClick={() => toggle(!open)}
        data-testid={testId && `${testId}-summary`}
        className={cx(r.summary.base, inGroup ? r.summary.inGroup : r.summary.alone)}
      >
        <text className={r.label.base}>{label}</text>
        <vector aria-hidden paths={glyphs.chevronDown} viewBox={glyphBox} className={cx(r.chevron.base, open && r.chevron.open)} />
      </button>
      {open && <view data-testid={testId && `${testId}-body`} className={cx(r.body.base, inGroup && r.body.inGroup)}>{children}</view>}
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
  className?: string;
  'data-testid'?: string;
}

/** Details rows in one bordered list: an accordion. */
export function DetailsGroup({ children, isExclusive = false, defaultValue, className, 'data-testid': testId }: DetailsGroupProps) {
  const open = useLocal<string | null>(defaultValue ?? null);
  const layer = useContext(LayerContext);
  return (
    <Group.Provider value={{ open: open.value, isExclusive, set: next => open.set(next) }}>
      <view data-testid={testId} className={cx(r.group.base, r.group.layer[layer], className)}>{children}</view>
    </Group.Provider>
  );
}
