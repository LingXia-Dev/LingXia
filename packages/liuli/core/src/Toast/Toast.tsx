import { createContext, cx, useContext, useId, useLocal, useView, type Env, type Local } from '@lingxia/view';
import type { BannerTone } from '../Banner/Banner';
import { Button } from '../Button/Button';
import { Glyph } from '../internal/Glyph';
import { toastRecipe as r } from './Toast.recipe';

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
  /** Replaces the toast with this id where it stands in the queue, instead of adding another. */
  id?: string;
  /** How long it shows, in ms (1 to 60000), over the region's `duration`; a toast with an action stays until closed. */
  duration?: number;
}

export interface Toaster {
  /** Queues a toast, or replaces the one with the same `id`; returns its id. */
  show(options: ToastOptions): string;
  /** Closes the toast with this id, shown or queued; without one, the toast showing. */
  dismiss(id?: string): void;
}

export interface ToastRegionProps {
  /** The dismiss button's name on every toast: "Dismiss". */
  dismissLabel: string;
  /** How long a toast without an action shows, in ms. @default 5000 */
  duration?: number;
  /** The page's content; `useToast()` anywhere inside it reaches this region. */
  children: JSX.Element;
}

export type ToastProps = {
  message: string;
  /** @default 'neutral' */
  tone?: ToastTone;
  action?: ToastAction;
  /** Shows a dismiss button. */
  onDismiss?: () => void;
  /** The dismiss button's name, with `onDismiss`. */
  dismissLabel?: string;
  className?: string;
  /** On the toast; the action gets `<testid>-action`, the dismiss button `<testid>-dismiss`. */
  'data-testid'?: string;
};

/** The toast as drawn. Pages show toasts through `ToastRegion` and `useToast()`; this is its face. */
export function Toast({ message, tone = 'neutral', action, onDismiss, dismissLabel, className, 'data-testid': testId }: ToastProps) {
  const glyph = toneGlyph[tone];
  return (
    <view data-testid={testId} className={cx(r.toast.base, onDismiss === undefined && r.toast.plain, className)}>
      {glyph && <Glyph name={glyph} className={cx(r.icon.base, r.icon.tone[tone])} />}
      <text className={r.message.base}>{message}</text>
      {action !== undefined && (
        <Button label={action.label} onClick={action.onClick} variant="link" size="sm" className={r.action.base} data-testid={testId && `${testId}-action`} />
      )}
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
  duration: number | null;
}

const outside: Toaster = {
  show() { throw Error('Liuli: useToast() needs a ToastRegion above it'); },
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
  const toaster: Toaster = {
    show(options) {
      const next = seq.value + 1;
      seq.set(next);
      const id = options.id ?? `toast-${next}`;
      const entry: Entry = {
        id,
        seq: next,
        message: options.message,
        tone: options.tone ?? 'neutral',
        action: options.action?.label ?? null,
        duration: options.duration ?? null,
      };
      if (options.action) actions.set(id, options.action.onClick);
      else actions.delete(id);
      const list = queue.value;
      const at = list.findIndex(item => item.id === id);
      update(at < 0 ? [...list, entry] : list.map((item, index) => (index === at ? entry : item)));
      return id;
    },
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

/** The nearest `ToastRegion`'s toaster. */
export function useToast(): Toaster {
  return useContext(ToastContext);
}

// The region fills the top layer's box absolutely, so the box's padding for the bars does not
// reach it; it clears them itself. The box already ends at the keyboard's top edge.
function clearBars(env: Env) {
  const gap = 16;
  return {
    paddingTop: gap + env.insets.top,
    paddingBottom: gap + Math.max(0, env.insets.bottom - env.keyboard.height),
    paddingLeft: gap + env.insets.left,
    paddingRight: gap + env.insets.right,
  };
}

const swipePoints = ['-120%', 0, '120%'] as const;

function Shown({ entry, regionId, dismissLabel, duration, toaster, actions }: { entry: Entry; regionId: string; dismissLabel: string; duration: number; toaster: Toaster; actions: Map<string, () => void> }) {
  const { env } = useView();
  // Rests on the middle point; a swipe to either side closes it.
  const swipe = useLocal('swipe', 1);
  const close = () => toaster.dismiss(entry.id);
  const run = actions.get(entry.id);
  // The host counts only while nothing holds the toast (a press, a resting pointer, focus inside)
  // and closes it through onToggle; a toast with an action waits to be closed.
  const closeAfter = entry.action !== null ? undefined : Math.min(60000, Math.max(1, Math.round(entry.duration ?? duration)));
  return (
    <view
      id={`${regionId}-${entry.seq}`}
      popover="manual"
      open
      closeAfter={closeAfter}
      onToggle={open => { if (!open) close(); }}
      className={cx(r.region.base, r.region.pointer[env.pointer])}
      style={clearBars(env)}
    >
      <view
        drag="x"
        dragSnapPoints={swipePoints}
        bind={swipe}
        aria-label={entry.message}
        onDragEnd={({ index }) => { if (index !== 1) close(); }}
        className={r.toast.pointer[env.pointer]}
      >
        <Toast
          message={entry.message}
          tone={entry.tone}
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
export function ToastRegion({ dismissLabel, duration = 5000, children }: ToastRegionProps) {
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
      <ToastContext.Provider value={toaster}>{children}</ToastContext.Provider>
      <text role="status" className={r.announcer.base}>{say(0, false)}</text>
      <text role="status" className={r.announcer.base}>{say(1, false)}</text>
      <text role="alert" className={r.announcer.base}>{say(0, true)}</text>
      <text role="alert" className={r.announcer.base}>{say(1, true)}</text>
      {head && <Shown key={`t${head.seq}`} entry={head} regionId={regionId} dismissLabel={dismissLabel} duration={duration} toaster={toaster} actions={actions} />}
    </>
  );
}
