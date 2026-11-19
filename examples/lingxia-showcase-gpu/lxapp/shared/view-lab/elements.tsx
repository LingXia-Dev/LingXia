// Drawing, media and control demos of the View catalogue: each is the body of one demo.
import { createContext, Fragment, List, useContext, useId, useLocal, useVideo, useView, useWebView, type Local, type VectorPath } from '@lingxia/view';
import { Button, Icon } from '../ui';
import type { LabPageData } from '../lab-types';
import { Readout, useSay } from './demo';
import { labels, readouts } from './text';
import { Term } from './text-input';
import { ProgressBar, ProgressRing } from './kit';

export function GradientsDemo() {
  const t = useSay();
  return (
    <>
      <view className="flex-row gap-3">
        <view data-testid="lab-gradient-radial" className="h-16 grow basis-0 rounded-xl bg-radial from-accent to-transparent" />
        <view data-testid="lab-gradient-conic" className="h-16 grow basis-0 rounded-xl bg-conic-90 from-accent via-danger to-accent" />
        <view data-testid="lab-gradient-arbitrary" className="h-16 grow basis-0 rounded-xl bg-radial-[at_top] from-surface to-accent to-70%" />
      </view>
      <view data-testid="lab-gradient-mask" className="h-16 rounded-xl bg-accent mask-b-from-20% mask-r-to-90% p-3">
        <text className="text-sm text-on-accent">{t(labels.maskText)}</text>
      </view>
      <view data-testid="lab-gradient-halo" className="h-20 items-center justify-center">
        <view className="absolute w-16 h-16 rounded-full bg-accent/40 blur-xl" />
        <view className="w-10 h-10 rounded-full bg-accent" />
      </view>
    </>
  );
}

export function BordersDemo() {
  return (
    <view className="flex-row gap-3">
      <view data-testid="lab-border-dashed" className="h-12 grow basis-0 rounded-xl border-2 border-dashed border-accent" />
      <view data-testid="lab-border-dotted" className="h-12 grow basis-0 rounded-full border-4 border-dotted border-danger" />
      <view data-testid="lab-border-edges" className="h-12 grow basis-0 border-t-2 border-l-4 border-dashed border-text" />
    </view>
  );
}

// A gap between a fill and its ring, a filled gap, an inset ring; the button and the switch ring
// only from a key. The second avatar's ring is the surface's colour, so it cuts into the first.
export function RingsDemo() {
  const on = useLocal('lab-ring-switch', false);
  const t = useSay();
  return (
    <>
      <view className="flex-row flex-wrap items-center gap-4 p-1">
        <view data-testid="lab-ring-gap" className="rounded-lg px-4 py-2 bg-accent ring-2 ring-offset-2 ring-accent"><text className="text-sm text-on-accent">{t(labels.ringGap)}</text></view>
        <view data-testid="lab-ring-band" className="rounded-lg px-4 py-2 bg-accent ring-2 ring-offset-2 ring-offset-surface-sunken ring-focus"><text className="text-sm text-on-accent">{t(labels.ringBand)}</text></view>
        <view data-testid="lab-ring-inset" className="rounded-lg px-4 py-2 bg-surface inset-ring-2 inset-ring-accent"><text className="text-sm text-text">{t(labels.ringInset)}</text></view>
      </view>
      <view className="flex-row flex-wrap items-center gap-4 p-1">
        <button data-testid="lab-ring-key" onClick={() => {}} className="rounded-lg px-4 py-2 bg-accent-solid text-on-accent-solid text-sm focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:ring-focus">{t(labels.ringKey)}</button>
        <switch data-testid="lab-ring-switch" aria-label="Thumb ring" bind={on} className="focus-visible:thumb:ring-2 focus-visible:thumb:ring-offset-2 focus-visible:thumb:ring-focus" />
        <view data-testid="lab-controls-avatars" className="flex-row">
          <view data-testid="lab-avatar-ada" role="img" aria-label="Ada Lovelace" className="size-10 rounded-full bg-accent items-center justify-center ring-2 ring-surface">
            <text className="text-on-accent text-sm">AL</text>
          </view>
          <view data-testid="lab-avatar-alan" role="img" aria-label="Alan Turing" className="size-10 -ml-3 rounded-full bg-success items-center justify-center ring-2 ring-surface">
            <text className="text-on-success text-sm">AT</text>
          </view>
        </view>
      </view>
    </>
  );
}

const star = 'M12 2l2.9 6.9 7.1.6-5.4 4.7 1.7 7-6.3-3.9-6.3 3.9 1.7-7L2 9.5l7.1-.6z';
const strokes: VectorPath[] = [
  { d: 'M3 6H21', fill: 'none', stroke: 'currentColor', strokeWidth: 3, strokeLinecap: 'round' },
  { d: 'M3 12H21', fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeDasharray: [3, 2], strokeDashoffset: 1 },
  { d: 'M3 21L12 15L21 21', fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinejoin: 'round', strokeLinecap: 'square' },
];

export function VectorsDemo() {
  return (
    <view className="flex-row gap-4 items-center">
      <vector data-testid="lab-vector-gradient" className="w-16 h-16" paths={[{ d: star, fill: 'url(#gold)', stroke: 'url(#edge)', strokeWidth: 0.75 }]}
        gradients={{
          gold: { type: 'radial', fx: 0.35, fy: 0.3, stops: [{ offset: 0, color: '#fff3b0' }, { offset: 1, color: '#f59e0b' }] },
          edge: { type: 'linear', x2: 0, y2: 1, stops: [{ offset: 0, color: '#b45309' }, { offset: 1, color: '#78350f', opacity: 0.6 }] },
        }} />
      <vector data-testid="lab-vector-strokes" className="w-16 h-16 text-text" paths={strokes} />
      <vector data-testid="lab-vector-conic" className="w-16 h-16" paths={[{ d: 'M12 2a10 10 0 1 0 0.01 0Z', fill: 'url(#wheel)' }]}
        gradients={{ wheel: { type: 'conic', stops: [{ offset: 0, color: '#ef4444' }, { offset: 0.33, color: '#22c55e' }, { offset: 0.66, color: '#3b82f6' }, { offset: 1, color: '#ef4444' }] } }} />
      <vector data-testid="lab-vector-transform" className="w-16 h-16 text-accent"
        paths={[0, 45, 90, 135].map(angle => ({ d: 'M12 4h1v16h-1z', transform: `rotate(${angle} 12.5 12)` }))} />
    </view>
  );
}

export function TransformsDemo() {
  return (
    <view className="flex-row gap-4 items-center">
      <Icon name="chevron" className="w-6 h-6 text-text rotate-90" />
      <Icon name="send" className="w-6 h-6 text-accent -scale-x-100" />
      <view className="relative w-16 h-10 rounded-lg bg-surface-sunken">
        <view data-testid="lab-transform-badge" className="absolute top-0 right-0 w-4 h-4 rounded-full bg-danger translate-x-1/2 -translate-y-1/2" />
      </view>
      <view data-testid="lab-transform-card" className="w-12 h-12 rounded-lg bg-accent origin-bottom-left rotate-12 scale-90" />
    </view>
  );
}

const clock = (seconds: number) => `${Math.floor(seconds / 60)}:${String(Math.floor(seconds % 60)).padStart(2, '0')}`;

// A player the page drives itself: one button, and the time read from the player. The clip
// ships with the app and rests on its poster until it plays.
export function VideoDemo() {
  const clip = useVideo('lab-clip');
  const t = useSay();
  const at = clock(clip.currentTime), of = clock(clip.duration ?? 0);
  return (
    <>
      <view className="rounded-xl overflow-hidden">
        <video data-testid="lab-video-player" player={clip} src="public/video/fractal.mp4" poster="public/video/fractal-poster.jpg" aria-label="Fractal zoom" loop />
      </view>
      <view className="flex-row items-center gap-3">
        <Button testId="lab-video-toggle" size="sm" onClick={() => (clip.paused ? clip.play() : clip.pause())}>{t(clip.paused ? labels.play : labels.pause)}</Button>
        <Readout testId="lab-video-state" state={`paused=${clip.paused}`}>{t(readouts.video(!clip.paused, at, of))}</Readout>
      </view>
    </>
  );
}

// A packaged HTML document shared between an inline card and a 70%-height `<dialog>` sheet without reloading.
export function WebViewDemo() {
  const web = useWebView('lab-web');
  const hash = useLocal('lab-webview-hash', '');
  const ackHash = useLocal('lab-webview-ack', '');
  const sheet = useLocal('lab-webview-sheet', false);
  const taps = useLocal('lab-webview-taps', 0);
  const t = useSay();
  return (
    <>
      <view className="relative h-36 rounded-xl overflow-hidden border border-line">
        <webview
          web={web}
          data-testid="lab-webview-frame"
          src={`public/embed/demo.html${hash.value}`}
          aria-label="Embedded demo page"
          onMessage={ev => {
            const msg = ev.data as { kind?: string; count?: number; hash?: string } | null;
            if (msg?.kind === 'tap' && typeof msg.count === 'number') taps.set(msg.count);
            else if (msg?.kind === 'hash' && typeof msg.hash === 'string') ackHash.set(msg.hash);
          }}
          className="w-full h-full rounded-xl"
        />
        <button
          data-testid="lab-webview-expand"
          aria-label={t(labels.expandWeb)}
          onClick={() => sheet.set(true)}
          className="absolute top-2.5 right-2.5 w-9 h-9 rounded-full bg-media text-on-media items-center justify-center"
        >
          <Icon name="fullscreen" viewBox={[0, 0, 24, 24]} className="w-4 h-4 text-on-media" />
        </button>
      </view>
      <view className="flex-row items-center gap-3">
        <Button testId="lab-webview-hash" size="sm" onClick={() => hash.set(hash.value === '#part-2' ? '#part-1' : '#part-2')}>
          {hash.value === '#part-2' ? '#part-1' : '#part-2'}
        </Button>
        <Readout testId="lab-webview-state" state={`title=${web.title} hash=${ackHash.value} taps=${taps.value} sheet=${sheet.value}`}>
          {`${web.title || 'loading'}${ackHash.value ? ` · ${ackHash.value}` : ''} · taps=${taps.value}`}
        </Readout>
      </view>
      <dialog
        open={sheet.value}
        closedby="any"
        onClose={() => sheet.set(false)}
        aria-label="Embedded demo sheet"
        data-testid="lab-webview-sheet"
        className="mt-auto mb-0 mx-0 w-full h-[70%] rounded-t-2xl bg-surface overflow-hidden"
      >
        <view className="flex-row items-center justify-between px-4 py-2.5 border-b border-line">
          <text className="text-sm font-semibold">{web.title || 'Embed Demo'}</text>
          <Button testId="lab-webview-sheet-close" size="sm" variant="secondary" onClick={() => sheet.set(false)}>
            {t(labels.close)}
          </Button>
        </view>
        <webview
          web={web}
          data-testid="lab-webview-sheet-frame"
          aria-label="Embedded demo page"
          className="w-full grow"
        />
      </dialog>
    </>
  );
}

type Pin = { x: number; y: number; moves: number; done: boolean };
const pinText = (pin: Pin | null) => (pin ? `${pin.x},${pin.y} moves=${pin.moves}${pin.done ? ' done' : ''}` : 'none');

// The same page twice, which cannot scroll: a drag on the first passes to the screen, the second
// keeps every drag that starts on it and moves its pin.
export function WebDragsDemo() {
  const plainWeb = useWebView('lab-web-plain');
  const ownWeb = useWebView('lab-web-own');
  const plain = useLocal<Pin | null>('lab-web-plain-pin', null);
  const own = useLocal<Pin | null>('lab-web-own-pin', null);
  const loaded = useLocal('lab-web-drags-loaded', 0);
  // The pages take the theme from the hash, which changes without reloading them.
  const src = `public/embed/pin.html#${useView().env.theme}`;
  const t = useSay();
  const take = (into: Local<Pin | null>) => (ev: { data: unknown }) => {
    const msg = ev.data as Partial<Pin> & { kind?: string } | null;
    if (msg?.kind === 'pin' && typeof msg.x === 'number' && typeof msg.y === 'number')
      into.set({ x: msg.x, y: msg.y, moves: msg.moves ?? 0, done: msg.done === true });
  };
  return (
    <>
      <text className="text-xs text-muted">{t(labels.webPlain)}</text>
      <webview web={plainWeb} data-testid="lab-web-drags-plain" src={src} aria-label={t(labels.webPlain)}
        onMessage={take(plain)} onLoad={() => loaded.set(loaded.value + 1)} className="w-full h-28 rounded-xl border border-line" />
      <text className="text-xs text-muted">{t(labels.webOwn)}</text>
      <webview web={ownWeb} data-testid="lab-web-drags-own" src={src} aria-label={t(labels.webOwn)} handlesDrags
        onMessage={take(own)} onLoad={() => loaded.set(loaded.value + 1)} className="w-full h-28 rounded-xl border border-line" />
      <view className="flex-row items-center gap-3">
        <Button testId="lab-web-drags-reset" size="sm" variant="secondary" onClick={() => { plain.set(null); own.set(null); }}>{t(labels.reset)}</Button>
        <Readout testId="lab-web-drags-state" state={`loaded=${loaded.value} plain=${pinText(plain.value)} own=${pinText(own.value)}`}>
          {t(readouts.pins(plain.value ? `${plain.value.x}, ${plain.value.y}` : '-', own.value ? `${own.value.x}, ${own.value.y}` : '-'))}
        </Readout>
      </view>
    </>
  );
}

// A page with no hash and no script for its theme: only `prefers-color-scheme`, which the web
// view takes from the View page's appearance. It reports what it reads, and each change.
export function WebSchemeDemo() {
  const web = useWebView('lab-web-scheme');
  const read = useLocal<{ scheme: string; changes: number; background: string } | null>('lab-web-scheme-read', null);
  const loads = useLocal('lab-web-scheme-loads', 0);
  const t = useSay();
  return (
    <>
      <webview web={web} data-testid="lab-web-scheme-frame" src="public/embed/scheme.html" aria-label={t(labels.webSchemeFrame)}
        onLoad={() => loads.set(loads.value + 1)}
        onMessage={(ev) => {
          const msg = ev.data as { kind?: string; scheme?: string; changes?: number; background?: string } | null;
          if (msg?.kind === 'scheme') read.set({ scheme: msg.scheme ?? '', changes: msg.changes ?? 0, background: msg.background ?? '' });
        }}
        className="w-full h-20 rounded-xl border border-line" />
      <Readout testId="lab-web-scheme-state"
        state={`loads=${loads.value} scheme=${read.value?.scheme ?? 'none'} changes=${read.value?.changes ?? 0} background=${read.value?.background ?? ''}`}>
        {t(readouts.scheme(read.value?.scheme ?? '-', read.value?.changes ?? 0))}
      </Readout>
    </>
  );
}

// A page taller than its box: a drag inside scrolls it to its edge; the next drag belongs to the screen.
export function WebScrollDemo() {
  const web = useWebView('lab-web-scroll');
  const at = useLocal<{ top: number; end: boolean } | null>('lab-web-scroll-at', null);
  const loads = useLocal('lab-web-scroll-loads', 0);
  const t = useSay();
  return (
    <>
      <webview web={web} data-testid="lab-web-scroll-frame" src="public/embed/scroll.html" aria-label={t(labels.webScrollFrame)}
        onLoad={() => loads.set(loads.value + 1)}
        onMessage={(ev) => {
          const msg = ev.data as { kind?: string; top?: number; end?: boolean } | null;
          if (msg?.kind === 'scroll' && typeof msg.top === 'number') at.set({ top: msg.top, end: msg.end === true });
        }}
        className="w-full h-40 rounded-xl border border-line" />
      <Readout testId="lab-web-scroll-state" state={`loads=${loads.value} top=${at.value?.top ?? -1} end=${at.value?.end === true}`}>
        {t(readouts.scrolled(at.value?.top ?? 0, at.value?.end === true))}
      </Readout>
    </>
  );
}

// Bar, pie and line plots drawn from the page's series; a bar takes the selection.
export function PlotDemo() {
  const { plots } = useView().data as unknown as LabPageData;
  const picked = useLocal('plots-picked', -1);
  const t = useSay();
  return (
    <>
      <plot data-testid="lab-plot-bar" kind="bar" values={plots.hourly} colors={['chart-1']} aria-label="Hourly series"
        selectedIndex={picked.value} onSelect={index => picked.set(index)} className="h-40" />
      <Readout testId="lab-plot-picked" state={`picked=${picked.value}`}>{t(readouts.plot(picked.value, plots.hourly[picked.value]))}</Readout>
      <view className="flex-row gap-4">
        <plot data-testid="lab-plot-pie" kind="pie" values={plots.share} colors={['chart-1', 'chart-2', 'chart-3', 'chart-4', 'chart-5']} aria-label="Share" className="grow basis-0 h-36" />
        <plot data-testid="lab-plot-line" kind="line" values={plots.latency} colors={['chart-2']} smooth area aria-label="Latency" className="grow basis-0 h-36" />
      </view>
    </>
  );
}

// A bar and a ring with a value a screen reader hears as a share, and both with none.
export function ProgressDemo() {
  const done = useLocal('progress-done', 40);
  const t = useSay();
  return (
    <>
      <ProgressBar value={done.value} label="Upload" testId="lab-progress-bar" />
      <ProgressBar label="Syncing" testId="lab-progress-busy" />
      <view className="flex-row gap-4 items-center">
        <ProgressRing value={done.value} label="Storage" valueText={`${done.value} of 100 GB`} testId="lab-progress-ring" />
        <ProgressRing label="Connecting" testId="lab-progress-spin" />
        <Button testId="lab-progress-more" size="sm" onClick={() => done.set(Math.min(100, done.value + 20))}>{t(labels.more)}</Button>
      </view>
    </>
  );
}

const field = 'px-3 py-2 rounded-lg border border-line-strong bg-surface text-sm';

// A radio group, a stepped slider, a slider styled by its parts, two switches and a growing field.
export function ControlsDemo() {
  const size = useLocal('controls-size', 'm');
  const volume = useLocal('controls-volume', 40);
  const scrub = useLocal('controls-scrub', 30);
  const power = useLocal('controls-power', false);
  const notes = useLocal('controls-notes', '');
  const t = useSay();
  const [small, medium, large, extra] = t(labels.sizes).split('|');
  return (
    <>
      <radio data-testid="lab-controls-size" bind={size} aria-label="Size" className="gap-1 text-sm"
        options={[{ value: 's', label: small, 'data-testid': 'lab-controls-size-s' }, { value: 'm', label: medium, 'data-testid': 'lab-controls-size-m' },
          { value: 'l', label: large, 'data-testid': 'lab-controls-size-l' }, { value: 'xl', label: extra, disabled: true, 'data-testid': 'lab-controls-size-xl' }]} />
      <slider data-testid="lab-controls-volume" bind={volume} min={0} max={100} step={10} aria-label="Volume" />
      <slider data-testid="lab-controls-scrub" bind={scrub} max={180} secondaryValue={Math.min(180, scrub.value + 45)} aria-label="Scrub"
        aria-valuetext={`${Math.floor(scrub.value / 60)}:${String(scrub.value % 60).padStart(2, '0')} of 3:00`}
        className="text-accent-solid px-0 track:h-[3px] active:track:h-[6px] thumb:size-0 active:thumb:size-4 focus-visible:thumb:size-4 focus-visible:thumb:ring-2 focus-visible:thumb:ring-accent/40" />
      <switch data-testid="lab-controls-power" bind={power} aria-label="Power" className="text-sm track:w-11 track:h-6 checked:track:bg-accent-solid thumb:size-5 active:thumb:w-6">{t(labels.power)}</switch>
      <switch data-testid="lab-controls-locked" value disabled onChange={() => {}} aria-label="Locked on" className="text-sm">{t(labels.lockedOn)}</switch>
      <textarea data-testid="lab-controls-notes" bind={notes} rows={3} maxRows={6} aria-label="Notes" placeholder={t(labels.severalLines)} className={field} />
      <Readout testId="lab-controls-values" state={`size=${size.value} volume=${volume.value} notes=${notes.value.length}`}>
        {t(readouts.controls(size.value, volume.value, notes.value.length))}
      </Readout>
    </>
  );
}

// One select with an option that cannot be chosen, and a narrow one whose long label stays on one line.
export function SelectDemo() {
  const plan = useLocal('plan', '');
  const billing = useLocal('controls-billing', 'q');
  const t = useSay();
  const [monthly, quarterly, yearly] = t(labels.plans).split('|');
  const [long, short] = t(labels.billings).split('|');
  return (
    <>
      <select data-testid="lab-input-plan" bind={plan} aria-label="Billing plan" placeholder={t(labels.choosePlan)} className={field}
        options={[{ value: 'monthly', label: monthly }, { value: 'quarterly', label: quarterly, disabled: true }, { value: 'yearly', label: yearly }]} />
      <select data-testid="lab-controls-billing" bind={billing} aria-label="Billing" className={`${field} w-40`}
        options={[{ value: 'q', label: long }, { value: 'm', label: short }]} />
      <Readout testId="lab-select-values" state={`plan=${plan.value} billing=${billing.value}`}>{t(readouts.select(plan.value, billing.value))}</Readout>
    </>
  );
}

// A compound control: the group provides size and selection; options read them, here as rows of an rtl List.
type ChoiceState = { size: 'sm' | 'md'; picked: Local<string> | null; menu: string };
const ChoiceGroup = createContext<ChoiceState>({ size: 'md', picked: null, menu: '' });
const choices = Object.freeze(['alpha', 'beta', 'gamma']);

export function SharedDemo() {
  const size = useLocal<'sm' | 'md'>('choices-size', 'md');
  const picked = useLocal('choices-picked', 'alpha');
  const menu = useId();
  const t = useSay();
  return (
    <>
      <view className="flex-row gap-2">
        <Button testId="lab-choices-size" size="sm" variant="secondary" onClick={() => size.set(size.value === 'md' ? 'sm' : 'md')}>{t(labels.changeSize)}</Button>
      </view>
      <view popover id={menu} aria-label="Choice menu" data-testid="lab-choices-menu" className="p-3 rounded-lg bg-surface border border-line">
        <text id="lab-choices-menu-text">{`menu for ${picked.value}`}</text>
      </view>
      <ChoiceGroup.Provider value={{ size: size.value, picked, menu }}>
        <view dir="rtl">
          <List data-testid="lab-choices-list" className="h-36 rounded-lg border border-line" items={choices} getKey={item => item}
            renderItem={item => <Choice value={item} />} />
        </view>
      </ChoiceGroup.Provider>
    </>
  );
}

function Choice({ value }: { value: string }) {
  const group = useContext(ChoiceGroup);
  const id = useId();
  const on = group.picked?.value === value;
  return (
    <view className="flex-row items-center border-b border-line">
      <button id={id} data-testid={`lab-choice-${value}`} onClick={() => group.picked?.set(value)}
        className={`grow flex-row justify-between px-3 ${group.size === 'sm' ? 'py-1 text-xs' : 'py-3 text-sm'} ${on ? 'bg-accent-soft' : ''}`}>
        <text data-testid={`lab-choice-${value}-label`}>{value}</text>
        <text data-testid={`lab-choice-${value}-state`}>{`${group.size}${on ? ' on' : ''}`}</text>
      </button>
      <button data-testid={`lab-choice-${value}-more`} popoverTarget={group.menu} aria-label={`More for ${value}`} className="px-3 py-2 text-sm">⋯</button>
    </view>
  );
}

// Fragments add no box: the terms sit directly in the column, and a keyed group moves as one.
export function PiecesDemo() {
  const words = useLocal('fragment-words', [['GPU', 'draws the page'], ['View', 'describes it'], ['Logic', 'owns the data']]);
  const note = useLocal('fragment-note', false);
  const t = useSay();
  return (
    <>
      <view className="flex-row gap-2">
        <Button testId="lab-fragments-rotate" size="sm" onClick={() => words.set([...words.value.slice(1), words.value[0]])}>{t(labels.rotate)}</Button>
        <Button testId="lab-fragments-note" size="sm" variant="secondary" onClick={() => note.set(!note.value)}>{t(note.value ? labels.hideNote : labels.showNote)}</Button>
      </view>
      <view data-testid="lab-fragments-list" className="gap-1">
        {note.value && <text data-testid="lab-fragments-hint" className="text-xs">{t(labels.pieceNote)}</text>}
        {words.value.map(([word, meaning]) => <Fragment key={word}><Term word={word} meaning={meaning} /></Fragment>)}
      </view>
    </>
  );
}

/** The Drawing, Media & data and Controls & composition groups: every demo the text table names, drawn by its body. */
export const drawingDemos = {
  gradients: GradientsDemo,
  borders: BordersDemo,
  rings: RingsDemo,
  vectors: VectorsDemo,
  transforms: TransformsDemo,
};
export const mediaDemos = {
  video: VideoDemo,
  webview: WebViewDemo,
  webScheme: WebSchemeDemo,
  webScroll: WebScrollDemo,
  webDrags: WebDragsDemo,
  plot: PlotDemo,
  progress: ProgressDemo,
};
export const controlDemos = {
  controls: ControlsDemo,
  select: SelectDemo,
  shared: SharedDemo,
  pieces: PiecesDemo,
};
