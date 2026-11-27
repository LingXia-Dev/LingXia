import { createContext, cx, errorText, useContext, useId, useLocal, useView, type Child, type Local } from '@lingxia/view';
import { liuliText } from '../i18n/text';
import type { BannerTone } from '../Banner/Banner';
import { Button } from '../Button/Button';
import { Glyph } from '../internal/Glyph';
import { useLargeText } from '../internal/largeText';
import { SpinnerMark } from '../internal/spinner';
import { FirstLine } from '../internal/firstLine';
import { toastRecipe as r } from './Toast.recipe';
import { dataOf } from '../internal/data';

export type ToastTone = BannerTone;

const toneGlyph = { neutral: null, info: 'info', success: 'success', warning: 'warning', danger: 'error' } as const;

export interface ToastAction {
  /** A verb for undoing or following up: "Undo", "View". */
  label: string;
  onClick: () => void;
}

export interface ToastOptions {
  /** What happened, in a few words: "Note archived". */
  message: string;
  /** @default 'neutral' */
  tone?: ToastTone;
  /** One follow-up; the toast closes after it runs. */
  action?: ToastAction;
  /** Work still under way: a spinner in the tone's glyph's place, reported busy, and no closing by itself; show the result with the same `id` to replace it. @default false */
  isBusy?: boolean;
  /** Replaces the toast with this id where it stands in the queue, instead of adding another. */
  id?: string;
  /** How long it shows, in ms (1 to 60000), over the region's `duration`; a toast with an action, or busy, stays until closed or replaced. */
  duration?: number;
}

/** A failure's toast: the message is its `errorText` words. */
export type ErrorToastOptions = Omit<ToastOptions, 'message'> & {
  /** The page's own words when the failure's code has none: "Couldn't save the note". */
  fallback?: string;
};

export interface Toaster {
  /** Queues a toast, or replaces the one with the same `id`; returns its id. */
  show(options: ToastOptions): string;
  /** Queues a failure as its `errorText` words, `danger` unless `tone` says otherwise; a cancel (`E_ABORT`) shows nothing and returns `null`. */
  show(error: unknown, options?: ErrorToastOptions): string | null;
  /** Closes the toast with this id, shown or queued; without one, the toast showing. */
  dismiss(id?: string): void;
}

/** The corner toasts show in. */
export type ToastPosition = 'bottomEnd' | 'bottomStart' | 'topEnd' | 'topStart';

export interface ToastRegionProps {
  /** The corner toasts show in: move them off controls the page keeps at the bottom end. @default 'bottomEnd' */
  position?: ToastPosition;
  /** Room the page keeps on an edge for a bar of its own, in pt, added to the 16 pt gap and the system bars. */
  inset?: { top?: number; bottom?: number; start?: number; end?: number };
  /** The dismiss button's name on every toast: "Dismiss". Defaults to "Dismiss" in the page's language. */
  dismissLabel?: string;
  /** How long a toast without an action, and not busy, shows, in ms. @default 5000 */
  duration?: number;
  /** The page's content, one element or several, a page-bottom `<scroll>` among them; `useToast()` anywhere inside it reaches this region. */
  children: Child;
}

export type ToastProps = {
  message: string;
  /** @default 'neutral' */
  tone?: ToastTone;
  action?: ToastAction;
  /** A spinner in the tone's glyph's place, and the toast reported busy. @default false */
  isBusy?: boolean;
  /** Shows a dismiss button. */
  onDismiss?: () => void;
  /** The dismiss button's name, with `onDismiss`. */
  dismissLabel?: string;
  className?: string;
  /** On the toast; the action gets `<testid>-action`, the dismiss button `<testid>-dismiss`. */
  'data-testid'?: string;
};

/** The toast as drawn. Pages show toasts through `ToastRegion` and `useToast()`; this is its face. */
export function Toast({ message, tone = 'neutral', action, isBusy = false, onDismiss, dismissLabel = liuliText('@liuli.dismiss'), className, 'data-testid': testId, ...attributes }: ToastProps) {
  const glyph = toneGlyph[tone];
  // At accessibility text sizes the action goes under the message, so the message keeps the card's width.
  const isUnder = useLargeText() && action !== undefined;
  const words = <text className={isUnder ? r.message.under : r.message.base}>{message}</text>;
  const follow = action !== undefined && (
    <Button label={action.label} onClick={action.onClick} variant="link" size="sm" className={r.action.base} data-testid={testId && `${testId}-action`} />
  );
  return (
    <view data-testid={testId} {...dataOf(attributes)} aria-busy={isBusy || undefined} className={cx(r.toast.base, onDismiss === undefined && r.toast.plain, className)}>
      {(isBusy || glyph) && (
        // One slot for the glyph and the spinner, on the message's first line.
        <FirstLine strut={r.message.type}>
          {isBusy
            ? <SpinnerMark side={20} className={cx(r.icon.base, r.icon.tone[tone], r.spinner.base)} />
            : glyph && <Glyph name={glyph} side={20} className={cx(r.icon.base, r.icon.tone[tone])} />}
        </FirstLine>
      )}
      {isUnder ? <view className={r.stack.base}>{words}{follow}</view> : <>{words}{follow}</>}
      {onDismiss !== undefined && (
        <button aria-label={dismissLabel} onClick={onDismiss} data-testid={testId && `${testId}-dismiss`} className={r.dismiss.base}>
          <Glyph name="close" className={r.dismissIcon.base} />
        </button>
      )}
    </view>
  );
}

interface Entry {
  id: string;
  /** Bumped on every show, so a replaced toast mounts afresh. */
  seq: number;
  message: string;
  tone: ToastTone;
  action: string | null;
  isBusy: boolean;
  duration: number | null;
}

const outside: Toaster = {
  show(): never { throw Error('Liuli: useToast() needs a ToastRegion above it'); },
  dismiss() {},
};
const ToastContext = createContext<Toaster>(outside);

// One toaster per region, kept with its queue cell so the context value stays the same object
// across renders; actions are functions, which local state cannot hold.
const regions = new WeakMap<object, { toaster: Toaster; actions: Map<string, () => void> }>();

function regionFor(queue: Local<Entry[]>, seq: Local<number>, slot: Local<number>) {
  const known = regions.get(queue);
  if (known) return known;
  const actions = new Map<string, () => void>();
  const update = (list: readonly Entry[]) => {
    if (list[0]?.seq !== queue.value[0]?.seq) slot.set(1 - slot.value);
    queue.set(list);
  };
  const add = (options: ToastOptions) => {
    const next = seq.value + 1;
    seq.set(next);
    const id = options.id ?? `toast-${next}`;
    const entry: Entry = {
      id,
      seq: next,
      message: options.message,
      tone: options.tone ?? 'neutral',
      action: options.action?.label ?? null,
      isBusy: options.isBusy ?? false,
      duration: options.duration ?? null,
    };
    if (options.action) actions.set(id, options.action.onClick);
    else actions.delete(id);
    const list = queue.value;
    const at = list.findIndex(item => item.id === id);
    update(at < 0 ? [...list, entry] : list.map((item, index) => (index === at ? entry : item)));
    return id;
  };
  const toaster: Toaster = {
    show: ((subject: unknown, options?: ErrorToastOptions) => {
      if (isToastOptions(subject)) return add(subject);
      const { fallback, ...rest } = options ?? {};
      const message = errorText(subject as { readonly code?: string } | null | undefined, fallback);
      return message === null ? null : add({ tone: 'danger', ...rest, message });
    }) as Toaster['show'],
    dismiss(id) {
      const list = queue.value;
      const target = id ?? list[0]?.id;
      if (target === undefined || !list.some(item => item.id === target)) return;
      actions.delete(target);
      update(list.filter(item => item.id !== target));
    },
  };
  const made = { toaster, actions };
  regions.set(queue, made);
  return made;
}

// A failure is an `Error` or carries a string `code`; the options form carries only `message`.
function isToastOptions(value: unknown): value is ToastOptions {
  if (value === null || typeof value !== 'object' || value instanceof Error) return false;
  const { code, message } = value as { code?: unknown; message?: unknown };
  return typeof code !== 'string' && typeof message === 'string';
}

/** The nearest `ToastRegion`'s toaster. */
export function useToast(): Toaster {
  return useContext(ToastContext);
}

const swipePoints = ['-120%', 0, '120%'] as const;
// The page's own bars, as margins of the region, so the layer still adds the system bars to its padding.
const edges = (inset: NonNullable<ToastRegionProps['inset']>) => {
  const { top, bottom, start, end } = inset;
  return {
    ...(top !== undefined && { marginTop: top }),
    ...(bottom !== undefined && { marginBottom: bottom }),
    ...(start !== undefined && { marginInlineStart: start }),
    ...(end !== undefined && { marginInlineEnd: end }),
  };
};

function Shown({ entry, regionId, position, inset, dismissLabel, duration, toaster, actions }: { entry: Entry; regionId: string; position: ToastPosition; inset?: ToastRegionProps['inset']; dismissLabel?: string; duration: number; toaster: Toaster; actions: Map<string, () => void> }) {
  // Rests on the middle point; a swipe to either side closes it.
  const swipe = useLocal('swipe', 1);
  const close = () => toaster.dismiss(entry.id);
  const run = actions.get(entry.id);
  // The host counts only while nothing holds the toast (a press, a resting pointer, focus inside)
  // and closes it through onToggle; a toast with an action, or busy, waits to be closed or replaced.
  const isTouch = useView().env.pointer !== 'fine';
  const closeAfter = entry.action !== null || entry.isBusy ? undefined : Math.min(60000, Math.max(1, Math.round(entry.duration ?? duration)));
  return (
    <view
      id={`${regionId}-${entry.seq}`}
      popover="manual"
      open
      closeAfter={closeAfter}
      onToggle={open => { if (!open) close(); }}
      style={inset && edges(inset)}
      className={cx(r.region.base, r.region.position[position], r.region.width[isTouch ? 'touch' : 'pointer'])}
    >
      <view
        drag="x"
        dragSnapPoints={swipePoints}
        bind={swipe}
        aria-label={entry.message}
        onDragEnd={({ index }) => { if (index !== 1) close(); }}
        className={r.toast.width}
      >
        <Toast
          message={entry.message}
          tone={entry.tone}
          isBusy={entry.isBusy}
          action={entry.action !== null && run ? { label: entry.action, onClick: () => { run(); close(); } } : undefined}
          onDismiss={close}
          dismissLabel={dismissLabel}
          data-testid="toast"
        />
      </view>
    </view>
  );
}

/**
 * Holds a page's toasts: one shows at a time, the rest wait in order. Put one at the root of a
 * page and reach it with `useToast()`. Each new message is announced, politely unless its tone is
 * danger.
 */
export function ToastRegion({ position = 'bottomEnd', inset, dismissLabel, duration = 5000, children }: ToastRegionProps) {
  const regionId = useId();
  const queue = useLocal<Entry[]>('toasts', []);
  const seq = useLocal('seq', 0);
  const slot = useLocal('slot', 0);
  const { toaster, actions } = regionFor(queue, seq, slot);
  const head = queue.value[0];
  const urgent = head?.tone === 'danger';
  // Each live text speaks only when its own text changes. A new toast takes the other one of each
  // pair, cleared when the last toast took its twin, so the same words twice are still announced.
  const say = (which: number, danger: boolean) => (head && urgent === danger && slot.value === which ? head.message : '');
  return (
    <>
      <ToastContext.Provider value={toaster}><view className={r.content.base}>{children}</view></ToastContext.Provider>
      <text role="status" className={r.announcer.base}>{say(0, false)}</text>
      <text role="status" className={r.announcer.base}>{say(1, false)}</text>
      <text role="alert" className={r.announcer.base}>{say(0, true)}</text>
      <text role="alert" className={r.announcer.base}>{say(1, true)}</text>
      {head && <Shown key={`t${head.seq}`} entry={head} regionId={regionId} position={position} inset={inset} dismissLabel={dismissLabel} duration={duration} toaster={toaster} actions={actions} />}
    </>
  );
}
