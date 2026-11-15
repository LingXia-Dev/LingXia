import { createContext, Details, Fragment, List, Pager as KitPagerView, Progress, reveal, Summary, useBox, useContext, useId, useLocal, useVideo, useView, type Local, type VectorPath } from '@lingxia/view';
import { VideoControls } from '@liulidesign/core/VideoControls';
import '../../theme/index.css';
import { Button, Icon, Page, Section } from '../../shared/ui';
import type { LabPageData } from '../../shared/lab-types';
import { TextLayoutDemo, RichTextDemo, DocumentDemo, SelectionDemo, TypeaheadDemo, FormSemanticsDemo, Term } from '../../shared/view-lab/text-input';
import { ScrubDemo, DragDemo, DragChainDemo, DetentsDemo, SwipeRowsDemo, PagerDemo, NestedControlsDemo } from '../../shared/view-lab/gestures';
import { PUBLIC_STREAM, PUBLIC_VIDEO } from '../../shared/public-video';
import { VIDEO_LABELS } from '../../shared/video-labels';

// GPU-only renderer fixtures: one section per View capability, addressed by testId.
// One group renders at a time: a whole-lab render exceeds the per-render budget on slower phones.
const groups = {
  paint: { label: 'Paint', sections: [Grid, Gradients, Vectors, Borders, Transforms] },
  layout: { label: 'Layout', sections: [Display, Sizing, Direction, Choices, Structure, StickyHeaders, Fragments, Disclosure] },
  text: { label: 'Text', sections: [TextLayout, RichText, Selection, Document] },
  input: { label: 'Input', sections: [Accessibility, Inputs, Controls, Focus, NestedControls, StateVariants, FormSemantics, Composites] },
  feedback: { label: 'Feedback', sections: [Overlays, Progresses] },
  media: { label: 'Media', sections: [Media] },
  data: { label: 'Data', sections: [Plots] },
  gesture: { label: 'Gesture', sections: [Drag, DragChain, Detents] },
  swipe: { label: 'Swipe', sections: [SwipeRows, Pager, KitPager] },
  engine: { label: 'Engine', sections: [Scrub, Typeahead, Appearance, Orphan, Reveal] },
};
type Group = keyof typeof groups;

export function render() {
  const group = useLocal<Group>('group', 'paint');
  // The bottom inset (bars and home indicator) and the keyboard height specs measure against.
  const { insets, safeArea, keyboard } = useView().env;
  const data = { 'data-inset-bottom': String(insets.bottom), 'data-safe-bottom': String(safeArea.bottom), 'data-keyboard': String(keyboard.height) } as const;
  return (
    <Page testId="view-lab" data={data}>
      <view className="flex-row gap-2 flex-wrap">
        {(Object.keys(groups) as Group[]).map(key => (
          <Button key={key} testId={`lab-tab-${key}`} size="sm" variant={group.value === key ? 'primary' : 'secondary'} onClick={() => group.set(key)}>
            {groups[key].label}
          </Button>
        ))}
      </view>
      {groups[group.value].sections.map((Part, index) => <Part key={`${group.value}-${index}`} />)}
    </Page>
  );
}

function Gradients() {
  return (
    <Section title="Gradients" testId="lab-gradients">
      <view className="flex-row gap-3">
        <view data-testid="lab-gradient-radial" className="h-16 grow basis-0 rounded-xl bg-radial from-accent to-transparent" />
        <view data-testid="lab-gradient-conic" className="h-16 grow basis-0 rounded-xl bg-conic-90 from-accent via-danger to-accent" />
        <view data-testid="lab-gradient-arbitrary" className="h-16 grow basis-0 rounded-xl bg-radial-[at_top] from-surface to-accent to-70%" />
      </view>
      <view data-testid="lab-gradient-mask" className="h-16 rounded-xl bg-accent mask-b-from-20% mask-r-to-90% p-3">
        <text className="text-sm text-on-accent">Fades out downward and to the right</text>
      </view>
      <view data-testid="lab-gradient-halo" className="h-20 items-center justify-center">
        <view className="absolute w-16 h-16 rounded-full bg-accent/40 blur-xl" />
        <view className="w-10 h-10 rounded-full bg-accent" />
      </view>
      <text data-testid="lab-gradient-text" className="text-3xl font-bold bg-clip-text text-transparent bg-linear-to-r from-accent to-danger">Gradient text</text>
    </Section>
  );
}

const star = 'M12 2l2.9 6.9 7.1.6-5.4 4.7 1.7 7-6.3-3.9-6.3 3.9 1.7-7L2 9.5l7.1-.6z';
const strokes: VectorPath[] = [
  { d: 'M3 6H21', fill: 'none', stroke: 'currentColor', strokeWidth: 3, strokeLinecap: 'round' },
  { d: 'M3 12H21', fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeDasharray: [3, 2], strokeDashoffset: 1 },
  { d: 'M3 21L12 15L21 21', fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinejoin: 'round', strokeLinecap: 'square' },
];

function Vectors() {
  return (
    <Section title="Vectors" testId="lab-vectors">
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
    </Section>
  );
}

function Borders() {
  return (
    <Section title="Borders" testId="lab-borders">
      <view className="flex-row gap-3">
        <view data-testid="lab-border-dashed" className="h-12 grow basis-0 rounded-xl border-2 border-dashed border-accent" />
        <view data-testid="lab-border-dotted" className="h-12 grow basis-0 rounded-full border-4 border-dotted border-danger" />
        <view data-testid="lab-border-edges" className="h-12 grow basis-0 border-t-2 border-l-4 border-dashed border-text" />
      </view>
    </Section>
  );
}

function Display() {
  const shown = useLocal('shown', true);
  return (
    <Section title="Display" testId="lab-display">
      <view className="flex-row gap-2 items-center">
        <Button testId="lab-display-toggle" onClick={() => shown.set(!shown.value)}>{shown.value ? 'Hide' : 'Show'}</Button>
        <text data-testid="lab-display-hidden" className={shown.value ? 'text-sm' : 'hidden'}>Toggled by hidden</text>
      </view>
      <view className="block gap-1">
        <text className="text-xs text-muted">block: a column that stretches</text>
        <view className="h-2 rounded bg-accent" />
      </view>
      <view className="flex-row h-16 items-center gap-2 bg-surface-sunken rounded-lg px-2">
        <view data-testid="lab-display-inline" className="inline-flex px-2 py-1 rounded bg-surface"><text className="text-xs">inline-flex</text></view>
        <view className="h-[8dvh] w-4 rounded bg-accent" />
      </view>
      <view data-testid="lab-display-fixed" className="fixed bottom-4 right-4 px-3 py-1 rounded-full bg-accent"><text className="text-xs text-on-accent">fixed</text></view>
    </Section>
  );
}

function Transforms() {
  return (
    <Section title="Transforms" testId="lab-transforms">
      <view className="flex-row gap-4 items-center">
        <Icon name="chevron" className="w-6 h-6 text-text rotate-90" />
        <Icon name="send" className="w-6 h-6 text-accent -scale-x-100" />
        <view className="relative w-16 h-10 rounded-lg bg-surface-sunken">
          <view data-testid="lab-transform-badge" className="absolute top-0 right-0 w-4 h-4 rounded-full bg-danger translate-x-1/2 -translate-y-1/2" />
        </view>
        <view data-testid="lab-transform-card" className="w-12 h-12 rounded-lg bg-accent origin-bottom-left rotate-12 scale-90" />
      </view>
    </Section>
  );
}

function TextLayout() {
  return <Section title="Text" testId="lab-text"><TextLayoutDemo /></Section>;
}

function Sizing() {
  return (
    <Section title="Sizing" testId="lab-sizing">
      <view className="flex-row gap-3 items-start">
        <view data-testid="lab-aspect-video" className="grow basis-0 aspect-video rounded-lg bg-accent" />
        <view data-testid="lab-aspect-square" className="w-16 aspect-square rounded-lg text-danger bg-current" />
      </view>
      <text data-testid="lab-max-ch" className="max-w-[24ch] text-sm select-none">This paragraph wraps at about twenty-four zeros of its own font.</text>
      <scroll data-testid="lab-overscroll" className="h-24 rounded-lg border border-line overscroll-contain">
        <view className="p-3 gap-2">
          {Array.from({ length: 8 }, (_, i) => <text key={String(i)} className="text-xs">Contained row {i + 1}</text>)}
        </view>
      </scroll>
    </Section>
  );
}

function Grid() {
  const tiles = ['Calls', 'Data', 'SMS', 'Roaming', 'Top up', 'Bills'];
  return (
    <Section title="Grid" testId="lab-grid">
      <view data-testid="lab-grid-auto-fit" className="grid grid-cols-[repeat(auto-fit,minmax(6rem,1fr))] gap-2">
        {tiles.map(tile => <view key={tile} className="h-14 rounded-lg bg-surface-sunken place-items-center grid"><text className="text-xs">{tile}</text></view>)}
      </view>
      <view data-testid="lab-grid-spans" className="grid grid-cols-3 grid-rows-[auto_auto] gap-2">
        <view className="col-span-2 h-12 rounded-lg bg-accent" />
        <view className="row-span-2 rounded-lg bg-danger" />
        <view className="h-12 rounded-lg bg-surface-sunken" />
        <view className="h-12 rounded-lg bg-surface-sunken order-first" />
      </view>
    </Section>
  );
}

function Direction() {
  const { env } = useView();
  return (
    <Section title={`Direction (env: ${env.direction})`} testId="lab-direction">
      {(['ltr', 'rtl'] as const).map(dir => (
        <view key={dir} dir={dir} data-testid={`lab-direction-${dir}`} className="flex-row items-center gap-2 ps-3 pe-1 py-2 rounded-s-xl border-s-4 border-accent bg-surface-sunken">
          <Icon name="chevron" className="w-5 h-5 text-text rtl:-scale-x-100" />
          <text className="grow basis-0 text-sm text-start">{dir === 'rtl' ? 'مرحبا بالعالم' : 'Hello, world'}</text>
          <text className="text-xs text-muted ms-auto">{dir}</text>
        </view>
      ))}
    </Section>
  );
}

function RichText() {
  return <Section title="Rich text" testId="lab-rich-text"><RichTextDemo /></Section>;
}

function Document() {
  return <Section title="Document" testId="lab-document"><DocumentDemo /></Section>;
}

// Bar, line and pie plots the renderer draws from Logic series; a bar takes the selection.
function Plots() {
  const { plots } = useView().data as unknown as LabPageData;
  const picked = useLocal('plots-picked', -1);
  const colors = ['chart-1', 'chart-2', 'chart-3', 'chart-4', 'chart-5'];
  return (
    <Section title="Plots" testId="lab-plots">
      <plot data-testid="lab-plot-bar" kind="bar" values={plots.hourly} colors={['chart-1']} aria-label="Hourly series"
        selectedIndex={picked.value} onSelect={index => picked.set(index)} className="h-40" />
      <text data-testid="lab-plot-picked" className="text-xs text-muted tabular-nums">{picked.value < 0 ? 'No bar selected' : `Hour ${picked.value}: ${plots.hourly[picked.value]}`}</text>
      <view className="flex-row gap-4">
        <plot data-testid="lab-plot-pie" kind="pie" values={plots.share} colors={colors} aria-label="Share" className="grow basis-0 h-36" />
        <plot data-testid="lab-plot-line" kind="line" values={plots.latency} colors={['chart-2']} smooth area aria-label="Latency" className="grow basis-0 h-36" />
      </view>
    </Section>
  );
}

// Kit progress: a bar and a ring with a value a screen reader hears as a share, and both indeterminate.
function Progresses() {
  const done = useLocal('progress-done', 40);
  return (
    <Section title="Progress" testId="lab-progress">
      <Progress value={done.value} aria-label="Upload" data-testid="lab-progress-bar" />
      <Progress aria-label="Syncing" data-testid="lab-progress-busy" />
      <view className="flex-row gap-4 items-center">
        <Progress variant="ring" value={done.value} aria-label="Storage" aria-valuetext={`${done.value} of 100 GB`} data-testid="lab-progress-ring" />
        <Progress variant="ring" aria-label="Connecting" data-testid="lab-progress-spin" />
        <Button testId="lab-progress-more" size="sm" onClick={() => done.set(Math.min(100, done.value + 20))}>More</Button>
      </view>
    </Section>
  );
}

function Media() {
  const { data, actions } = useView();
  const clip = useVideo('lab-clip');
  const stream = useVideo('lab-stream');
  // Specs pass `media=packaged` for the bundled clip; a network failure falls back to it too.
  const packaged = data.packagedMedia === true || clip.error?.code === 'VIDEO_NETWORK';
  const source = packaged
    ? { src: 'public/video/fractal.mp4', poster: 'public/video/fractal-poster.jpg', 'aria-label': 'Fractal zoom' }
    : { sources: PUBLIC_VIDEO, 'aria-label': 'Big Buck Bunny' };
  // Waits for a tap; the kit overlay drives it, its menu picking the quality.
  return (
    <Section title="Media" testId="lab-video">
      <view className="relative rounded-xl overflow-hidden">
        <video data-testid="lab-video-player" player={clip} {...source} loop={!packaged} />
        <VideoControls player={clip} labels={VIDEO_LABELS} poster={packaged ? 'public/video/fractal-poster.jpg' : undefined} data-testid="lab-video-controls" />
      </view>
      {/* Specs only: a spinner beside the player commits the page every frame; the bar must still follow it. */}
      {packaged ? <Progress variant="ring" size={24} aria-label="Loading next" data-testid="lab-video-spinner" /> : (
        <view className="gap-2">
          <text className="text-sm text-muted">HLS: the stream's renditions are its qualities.</text>
          <view className="relative rounded-xl overflow-hidden">
            <video data-testid="lab-stream-player" player={stream} src={PUBLIC_STREAM} aria-label="HLS test stream" />
            <VideoControls player={stream} labels={VIDEO_LABELS} data-testid="lab-stream-controls" />
          </view>
        </view>
      )}
      <Button testId="lab-video-page" variant="secondary" onClick={() => actions.openVideoPage()}>
        Video page: quality, HLS and a list of players
      </Button>
    </Section>
  );
}

function Accessibility() {
  const saves = useLocal('saves', 0);
  return (
    <Section title="Accessibility" testId="lab-accessibility">
      <text data-testid="lab-a11y-heading" role="heading" headingLevel={3} className="text-base font-semibold">Monthly bills</text>
      <view data-testid="lab-a11y-list" role="list" aria-label="Bills" className="gap-1">
        {['Water', 'Power', 'Internet'].map(bill => (
          <view key={bill} role="listitem" className="flex-row justify-between">
            <text className="text-sm">{bill}</text>
            <vector aria-hidden className="w-4 h-4 text-muted" paths={[{ d: 'M9 18l6-6-6-6', fill: 'none', stroke: 'currentColor', strokeWidth: 1.5 }]} />
          </view>
        ))}
      </view>
      <view className="flex-row items-center gap-3">
        <Button testId="lab-a11y-save" onClick={() => saves.set(saves.value + 1)}>Save</Button>
        <text data-testid="lab-a11y-status" role="status" className="text-sm text-muted">{saves.value ? `Saved ${saves.value} times` : 'Not saved yet'}</text>
      </view>
    </Section>
  );
}

function Inputs() {
  const code = useLocal('code', '');
  const amount = useLocal('amount', '');
  const day = useLocal('day', '');
  const time = useLocal('time', '');
  const plan = useLocal('plan', '');
  const field = 'px-3 py-2 rounded-lg border border-line-strong bg-surface text-sm';
  return (
    <Section title="Inputs" testId="lab-inputs">
      <input data-testid="lab-input-otp" bind={code} aria-label="One-time code" type="number" maxLength={6} autoComplete="one-time-code" enterKeyHint="done" placeholder="6-digit code" className={field} />
      <input data-testid="lab-input-amount" bind={amount} aria-label="Amount" type="number" min={1} max={500} step={0.5} enterKeyHint="next" placeholder="1 to 500, steps of 0.5" className={field} />
      <view className="flex-row gap-2">
        <input data-testid="lab-input-date" bind={day} aria-label="Billing date" type="date" min="2024-01-01" placeholder="Pick a date" className={`grow basis-0 ${field}`} />
        <input data-testid="lab-input-time" bind={time} aria-label="Reminder time" type="time" placeholder="Pick a time" className={`grow basis-0 ${field}`} />
      </view>
      <select data-testid="lab-input-plan" bind={plan} aria-label="Billing plan" placeholder="Choose a plan" className={field}
        options={[{ value: 'monthly', label: 'Monthly' }, { value: 'quarterly', label: 'Quarterly', disabled: true }, { value: 'yearly', label: 'Yearly' }]} />
      <input data-testid="lab-input-name" aria-label="Name" autoComplete="name" autoCapitalize="words" spellCheck={false} placeholder="Full name" className={field} />
      <text data-testid="lab-input-values" className="text-xs text-muted">{`code=${code.value} amount=${amount.value} date=${day.value} time=${time.value} plan=${plan.value}`}</text>
    </Section>
  );
}

function Structure() {
  const rows = ['Starter', 'Plus', 'Pro', 'Max'];
  return (
    <Section title="Structural variants" testId="lab-structure">
      <view data-testid="lab-structure-rows" className="rounded-lg border border-line">
        {rows.map(row => (
          <text key={row} className="px-3 py-2 text-sm odd:bg-surface-sunken first:rounded-t-lg last:rounded-b-lg last:font-semibold">{row}</text>
        ))}
      </view>
      <view className="flex-row gap-2">
        <view data-testid="lab-structure-empty" className="h-8 grow basis-0 rounded-lg bg-danger empty:bg-surface-sunken" />
        <view className="h-8 grow basis-0 rounded-lg bg-surface-sunken"><text className="only:text-xs only:p-2">only child</text></view>
      </view>
    </Section>
  );
}

function StickyHeaders() {
  const groups = { Today: ['Top up', 'Data pack'], Yesterday: ['Bill paid', 'Roaming on', 'SMS bundle'], 'Last week': ['Plan change', 'Refund'] };
  return (
    <Section title="Sticky headers" testId="lab-sticky">
      <scroll data-testid="lab-sticky-scroll" className="h-48 rounded-lg border border-line">
        {Object.entries(groups).map(([day, items]) => (
          <view key={day}>
            <text className="sticky top-0 px-3 py-1.5 text-xs font-semibold bg-surface-sunken">{day}</text>
            {items.map(item => <text key={item} className="px-3 py-3 text-sm border-b border-line">{item}</text>)}
          </view>
        ))}
      </scroll>
    </Section>
  );
}

function Selection() {
  return <Section title="Selectable text" testId="lab-selection"><SelectionDemo /></Section>;
}

function Focus() {
  const open = useLocal('dialog-open', false);
  const choice = useLocal('dialog-choice', 'none');
  const close = (value: string) => { choice.set(value); open.set(false); };
  // Pointer crossings on nested boxes, as a sequence a spec can read: O = outer box, I = inner button.
  const crossings = useLocal('lab-crossings', '');
  const cross = (mark: string) => () => crossings.set(crossings.value + mark);
  return (
    <Section title="Keyboard focus" testId="lab-focus">
      <text className="text-xs text-muted">With a keyboard: Tab to Open, press Enter; focus stays in the dialog and returns to Open when it closes.</text>
      <view className="flex-row gap-2 items-center">
        <Button testId="lab-focus-open" onClick={() => open.set(true)}>Open dialog</Button>
        <button data-testid="lab-focus-visible" onClick={() => {}} className="rounded-xl px-4 py-2 bg-surface-sunken focus-visible:bg-accent focus-visible:text-on-accent">Ring by key</button>
        <button data-testid="lab-interest" interestFor="lab-interest-hint" aria-label="About focus" className="rounded-full w-8 h-8 items-center justify-center bg-surface-sunken focus-visible:bg-accent focus-visible:text-on-accent"><text className="text-sm">?</text></button>
        <text data-testid="lab-focus-choice" className="text-sm">{`chosen: ${choice.value}`}</text>
      </view>
      <view id="lab-interest-hint" popover="hint" role="tooltip" data-testid="lab-interest-hint" className="max-w-56 px-3 py-2 rounded-lg border border-line bg-surface-raised">
        <text className="text-xs text-text">Rest the pointer here, or Tab to it, to read this.</text>
      </view>
      <view data-testid="lab-crossing" onPointerEnter={cross('O+')} onPointerLeave={cross('O-')} className="flex-row items-center gap-3 p-3 rounded-xl border border-line">
        <button data-testid="lab-crossing-inner" onPointerEnter={cross('I+')} onPointerLeave={cross('I-')} onClick={() => crossings.set('')} className="rounded-lg px-3 py-1.5 bg-surface-sunken hover:bg-accent-soft text-sm">Clear</button>
        <text data-testid="lab-crossing-log" className="text-xs text-muted">{crossings.value || 'Move a mouse over this box'}</text>
      </view>
      <dialog open={open.value} onClose={() => close('escape')} data-testid="lab-focus-dialog" aria-label="Confirm payment" className="w-72 gap-3 p-4 rounded-xl border border-line bg-surface">
        <text role="heading" headingLevel={3} className="text-sm font-semibold">Confirm payment</text>
        <view className="flex-row gap-2">
          <Button testId="lab-focus-confirm" onClick={() => close('confirm')}>Confirm</Button>
          <Button testId="lab-focus-cancel" variant="secondary" onClick={() => close('cancel')}>Cancel</Button>
        </view>
      </dialog>
    </Section>
  );
}

function NestedControls() {
  return <Section title="Nested controls" testId="lab-nested"><NestedControlsDemo /></Section>;
}

// Fragments add no box: the terms sit directly in the column, and a keyed group moves as one.
function Fragments() {
  const words = useLocal('fragment-words', [['GPU', 'draws the page'], ['View', 'describes it'], ['Logic', 'owns the data']]);
  const note = useLocal('fragment-note', false);
  return (
    <Section title="Fragments" testId="lab-fragments">
      <view className="flex-row gap-2">
        <Button testId="lab-fragments-rotate" size="sm" onClick={() => words.set([...words.value.slice(1), words.value[0]])}>Rotate</Button>
        <Button testId="lab-fragments-note" size="sm" onClick={() => note.set(!note.value)}>{note.value ? 'Hide note' : 'Show note'}</Button>
      </view>
      <view data-testid="lab-fragments-list" className="gap-1">
        {note.value && <text data-testid="lab-fragments-hint" className="text-xs">Each term is two texts in one fragment.</text>}
        {words.value.map(([word, meaning]) => <Fragment key={word}><Term word={word} meaning={meaning} /></Fragment>)}
      </view>
    </Section>
  );
}

function FormSemantics() {
  return <Section title="Form semantics" testId="lab-form"><FormSemanticsDemo /></Section>;
}

// Tabs, a toolbar and a tree: one Tab stop each, arrows move between items, tabs activate on arrival.
function Composites() {
  const range = useLocal('composite-range', 'week');
  const bold = useLocal('composite-bold', false);
  const open = useLocal('composite-open', true);
  const tab = (value: string, label: string) => (
    <button key={value} role="tab" aria-selected={range.value === value} data-testid={`lab-range-${value}`} onClick={() => range.set(value)}
      className={`px-3 py-1 rounded-lg text-sm ${range.value === value ? 'bg-accent text-on-accent' : 'bg-surface-sunken'}`}>{label}</button>
  );
  const node = (label: string, level: number, testId: string, expanded?: boolean) => (
    <button role="treeitem" aria-level={level} aria-expanded={expanded} data-testid={testId}
      onClick={expanded === undefined ? () => {} : () => open.set(!open.value)} className="text-sm text-left py-1" style={{ paddingLeft: level * 12 }}>{label}</button>
  );
  return (
    <Section title="Composites" testId="lab-composites">
      <view role="tablist" aria-label="Range" className="flex-row gap-2">
        {tab('day', 'Day')}{tab('week', 'Week')}{tab('month', 'Month')}
      </view>
      <view role="tabpanel" aria-label="Range view" data-testid="lab-range-panel"><text className="text-sm">{`Showing the ${range.value}`}</text></view>
      <view role="toolbar" aria-label="Format" className="flex-row gap-2">
        <button data-testid="lab-tool-bold" aria-pressed={bold.value} onClick={() => bold.set(!bold.value)} className="px-3 py-1 rounded-lg bg-surface-sunken text-sm">Bold</button>
        <button data-testid="lab-tool-copy" onClick={() => {}} className="px-3 py-1 rounded-lg bg-surface-sunken text-sm">Copy</button>
      </view>
      <view role="tree" aria-label="Files" className="gap-0">
        {node('src', 1, 'lab-tree-src', open.value)}
        {open.value ? node('main.ts', 2, 'lab-tree-main') : null}
        {node('docs', 1, 'lab-tree-docs')}
      </view>
    </Section>
  );
}

// `data-*` reads the element's own props; `has-*` and `focus-within:` follow what is below it.
function StateVariants() {
  const agreed = useLocal('variants-agreed', false);
  const open = useLocal('variants-open', false);
  const note = useLocal('variants-note', '');
  return (
    <Section title="Data and has variants" testId="lab-variants">
      <view data-testid="lab-variants-agree" className="flex-row items-center gap-3 rounded-xl px-4 py-3 bg-surface-sunken has-checked:bg-accent has-checked:text-on-accent">
        <checkbox data-testid="lab-variants-checkbox" bind={agreed} aria-label="Agree" />
        <text className="text-sm">{agreed.value ? 'Agreed' : 'Tap to agree'}</text>
      </view>
      <button data-testid="lab-variants-toggle" data-state={open.value ? 'open' : 'closed'} onClick={() => open.set(!open.value)}
        className="rounded-xl px-4 py-3 bg-surface-sunken data-[state=open]:bg-accent data-[state=open]:text-on-accent data-[state=open]:py-5">
        {open.value ? 'Open (taller)' : 'Closed'}
      </button>
      <view data-testid="lab-variants-field" className="rounded-xl p-3 gap-1 border border-line focus-within:border-accent focus-within:bg-surface-sunken">
        <text className="text-xs text-muted">Note</text>
        <input data-testid="lab-variants-input" bind={note} placeholder="Focus me" aria-label="Note" />
      </view>
    </Section>
  );
}
// A compound control: the group provides size and selection; options read them, here as rows of an rtl List.
type ChoiceState = { size: 'sm' | 'md'; picked: Local<string> | null; menu: string; clash: boolean };
const ChoiceGroup = createContext<ChoiceState>({ size: 'md', picked: null, menu: '', clash: false });
const choices = Object.freeze(['alpha', 'beta', 'gamma']);

function Choices() {
  const size = useLocal<'sm' | 'md'>('choices-size', 'md');
  const picked = useLocal('choices-picked', 'alpha');
  // An id clash inside a list row fails the render with UI_ID.
  const clash = useLocal('choices-clash', false);
  const menu = useId();
  return (
    <Section title="Context" testId="lab-choices">
      <view className="flex-row gap-2">
        <Button testId="lab-choices-size" size="sm" variant="secondary" onClick={() => size.set(size.value === 'md' ? 'sm' : 'md')}>Toggle size</Button>
        <Button testId="lab-choices-clash" size="sm" variant="secondary" onClick={() => clash.set(true)}>Clash ids</Button>
      </view>
      <view popover id={menu} aria-label="Choice menu" data-testid="lab-choices-menu" className="p-3 rounded-lg bg-surface border border-line">
        <text id="lab-choices-menu-text">{`menu for ${picked.value}`}</text>
      </view>
      <ChoiceGroup.Provider value={{ size: size.value, picked, menu, clash: clash.value }}>
        <view dir="rtl">
          <List data-testid="lab-choices-list" className="h-36 rounded-lg border border-line" items={choices} getKey={item => item}
            renderItem={item => <Choice value={item} />} />
        </view>
      </ChoiceGroup.Provider>
    </Section>
  );
}

function Choice({ value }: { value: string }) {
  const group = useContext(ChoiceGroup);
  const id = useId();
  const on = group.picked?.value === value;
  return (
    <view className="flex-row items-center border-b border-line">
      <button id={group.clash && value === 'gamma' ? 'lab-choices-menu-text' : id} data-testid={`lab-choice-${value}`} onClick={() => group.picked?.set(value)}
        className={`grow flex-row justify-between px-3 ${group.size === 'sm' ? 'py-1 text-xs' : 'py-3 text-sm'} ${on ? 'bg-accent-soft' : ''}`}>
        <text data-testid={`lab-choice-${value}-label`}>{value}</text>
        <text data-testid={`lab-choice-${value}-state`}>{`${group.size}${on ? ' on' : ''}`}</text>
      </button>
      <button data-testid={`lab-choice-${value}-more`} popoverTarget={group.menu} aria-label={`More for ${value}`} className="px-3 py-2 text-sm">⋯</button>
    </view>
  );
}

// Frozen rows in a frozen array: a List reuses the rows it already rendered.
const rows = Object.freeze(Array.from({ length: 40 }, (_, index) => Object.freeze({ id: `row-${index + 1}`, label: `Row ${index + 1}` })));

// Every control kind a spec drives: radio group, slider, multi-line editor, search field and a keyed list.
function Controls() {
  const size = useLocal('controls-size', 'm');
  const volume = useLocal('controls-volume', 40);
  const scrub = useLocal('controls-scrub', 30);
  const power = useLocal('controls-power', false);
  const billing = useLocal('controls-billing', 'q');
  const notes = useLocal('controls-notes', '');
  const query = useLocal('controls-query', '');
  const picked = useLocal('controls-picked', '');
  const field = 'px-3 py-2 rounded-lg border border-line-strong bg-surface text-sm';
  return (
    <Section title="Controls" testId="lab-controls">
      <radio data-testid="lab-controls-size" bind={size} aria-label="Size" className="gap-1 text-sm"
        options={[{ value: 's', label: 'Small', 'data-testid': 'lab-controls-size-s' }, { value: 'm', label: 'Medium', 'data-testid': 'lab-controls-size-m' },
          { value: 'l', label: 'Large', 'data-testid': 'lab-controls-size-l' }, { value: 'xl', label: 'Extra large', disabled: true, 'data-testid': 'lab-controls-size-xl' }]} />
      <slider data-testid="lab-controls-volume" bind={volume} min={0} max={100} step={10} aria-label="Volume" className="text-accent" />
      <slider data-testid="lab-controls-scrub" bind={scrub} max={180} secondaryValue={Math.min(180, scrub.value + 45)} aria-label="Scrub"
        aria-valuetext={`${Math.floor(scrub.value / 60)}:${String(scrub.value % 60).padStart(2, '0')} of 3:00`}
        className="text-accent px-0 track:h-[3px] active:track:h-[6px] thumb:size-0 active:thumb:size-4 focus-visible:thumb:size-4 focus-visible:thumb:ring-2 focus-visible:thumb:ring-accent/40" />
      <switch data-testid="lab-controls-power" bind={power} className="text-sm track:w-11 track:h-6 checked:track:bg-success thumb:size-5 active:thumb:w-6">Power</switch>
      <switch data-testid="lab-controls-locked" value disabled onChange={() => {}} className="text-sm">Locked on</switch>
      <select data-testid="lab-controls-billing" bind={billing} aria-label="Billing" className={`${field} w-40`}
        options={[{ value: 'q', label: 'Quarterly billing with the annual discount applied' }, { value: 'm', label: 'Monthly' }]} />
      <view data-testid="lab-controls-avatars" className="flex-row p-3 bg-surface rounded-lg">
        <view data-testid="lab-avatar-ada" role="img" aria-label="Ada Lovelace" className="size-10 rounded-full bg-accent items-center justify-center ring-2 ring-surface">
          <text className="text-on-accent text-sm">AL</text>
        </view>
        <view data-testid="lab-avatar-alan" role="img" aria-label="Alan Turing" className="size-10 -ml-3 rounded-full bg-success items-center justify-center ring-2 ring-surface">
          <text className="text-on-success text-sm">AT</text>
        </view>
      </view>
      <textarea data-testid="lab-controls-notes" bind={notes} rows={3} maxRows={6} aria-label="Notes" placeholder="Several lines" className={field} />
      <input type="search" data-testid="lab-controls-search" bind={query} aria-label="Filter rows" placeholder="Filter rows" className={field} />
      <List data-testid="lab-controls-list" className="h-48 rounded-lg border border-line"
        items={Object.freeze(rows.filter(row => row.label.toLowerCase().includes(query.value.toLowerCase())))} getKey={row => row.id}
        renderItem={row => (
          <button data-testid={`lab-controls-${row.id}`} onClick={() => picked.set(row.label)} className="px-3 py-2 border-b border-line text-sm">{row.label}</button>
        )} />
      <text data-testid="lab-controls-values" className="text-xs text-muted tabular-nums">{`size=${size.value} volume=${volume.value} notes=${notes.value.length} query=${query.value} picked=${picked.value}`}</text>
    </Section>
  );
}

// HTML's details/summary: the summary announces expanded; the body exists only while open.
function Disclosure() {
  return (
    <Section title="Disclosure" testId="lab-disclosure">
      <Details data-testid="lab-disclosure-licenses">
        <Summary className="py-2 text-sm font-semibold">Open-source licenses</Summary>
        <text data-testid="lab-disclosure-body" className="text-sm text-muted">MIT, Apache-2.0, BSD-3-Clause</text>
      </Details>
    </Section>
  );
}

// Dialogs and popovers draw in the top layer: above the page and unclipped by this section.
function Overlays() {
  const confirm = useLocal('overlay-confirm', false);
  const sheet = useLocal('overlay-sheet', false);
  const last = useLocal('overlay-last', 'none');
  const act = (value: string) => last.set(value);
  // The sheet covers the bars; its content clears the home indicator or gesture bar.
  const safeBottom = useView().env.safeArea?.bottom ?? 0;
  return (
    <Section title="Overlays" testId="lab-overlays">
      <view className="flex-row gap-2 items-center flex-wrap">
        <Button testId="lab-overlay-confirm" onClick={() => confirm.set(true)}>Dialog</Button>
        <Button testId="lab-overlay-sheet" variant="secondary" onClick={() => sheet.set(true)}>Sheet</Button>
        <button popoverTarget="lab-menu" data-testid="lab-overlay-more" aria-label="More" className="ms-auto rounded-full px-3 py-1.5 bg-surface-sunken text-text">⋯</button>
      </view>
      <text data-testid="lab-overlay-last" className="text-sm">{`last: ${last.value}`}</text>
      <view id="lab-menu" popover role="menu" aria-label="Note actions" data-testid="lab-menu"
        style={{ positionArea: 'bottom span-inline-start' }} className="w-44 p-1 rounded-xl bg-surface-raised shadow-lg border border-line">
        {['Rename', 'Share', 'Delete'].map(item => (
          <button key={item} role="menuitem" data-testid={`lab-menu-${item.toLowerCase()}`} onClick={() => act(item)}
            className="px-3 py-2 rounded-lg text-start text-text active:bg-surface-sunken">{item}</button>
        ))}
      </view>
      <dialog open={confirm.value} onClose={() => confirm.set(false)} aria-label="Delete note?" data-testid="lab-dialog"
        className="w-72 p-4 gap-3 rounded-2xl bg-surface-raised transition duration-200 starting:opacity-0 starting:scale-95 exit:opacity-0 exit:scale-95">
        <text role="heading" headingLevel={3} className="text-base font-semibold">Delete note?</text>
        <text className="text-sm text-muted">Escape cancels; the backdrop does not.</text>
        <view className="flex-row gap-2 justify-end">
          <Button testId="lab-dialog-cancel" variant="secondary" onClick={() => { act('cancel'); confirm.set(false); }}>Cancel</Button>
          <Button testId="lab-dialog-delete" onClick={() => { act('delete'); confirm.set(false); }}>Delete</Button>
        </view>
      </dialog>
      <dialog open={sheet.value} onClose={() => sheet.set(false)} closedby="any" aria-label="Filters" data-testid="lab-sheet"
        style={{ paddingBottom: 16 + safeBottom }}
        className="mt-auto mb-0 w-full p-4 gap-3 rounded-t-3xl bg-surface-raised transition-transform duration-300 ease-out starting:translate-y-full exit:translate-y-full">
        <view className="self-center w-10 h-1 rounded-full bg-line-strong" />
        <text role="heading" headingLevel={3} className="text-base font-semibold">Filters</text>
        <scroll data-testid="lab-sheet-scroll" className="max-h-48">
          {Array.from({ length: 12 }, (_, i) => <text key={`f${i}`} className="py-2 text-sm">{`Filter ${i + 1}`}</text>)}
        </scroll>
        <Button testId="lab-sheet-apply" onClick={() => { act('apply'); sheet.set(false); }}>Apply</Button>
      </dialog>
    </Section>
  );
}

function Scrub() {
  return <Section title="Scrub" testId="lab-scrub"><ScrubDemo /></Section>;
}

function Typeahead() {
  return <Section title="Typeahead" testId="lab-type"><TypeaheadDemo /></Section>;
}

// Counts this page's View runs: a theme change repaints a page that never read the appearance
// without running it, and runs one that did.
let labRuns = 0;
function Appearance() {
  const reads = useLocal('lab-appearance-reads', false);
  const { env } = useView();
  labRuns += 1;
  return (
    <Section title="Appearance" testId="lab-appearance">
      <text data-testid="lab-appearance-runs" className="text-sm tabular-nums">{`runs ${labRuns}`}</text>
      <text data-testid="lab-appearance-theme" className="text-sm text-muted">{reads.value ? `theme ${env.theme}` : 'theme not read'}</text>
      <Button testId="lab-appearance-toggle" size="sm" variant="secondary" onClick={() => reads.set(!reads.value)}>
        {reads.value ? 'Stop reading the theme' : 'Read the theme'}
      </Button>
    </Section>
  );
}

// A date field that leaves the page while its picker is open: the picker goes with it.
function Orphan() {
  const shown = useLocal('lab-orphan-shown', true);
  const day = useLocal('lab-orphan-day', '2026-10-21');
  return (
    <Section title="Orphan overlay" testId="lab-orphan">
      {shown.value
        ? <input data-testid="lab-orphan-date" bind={day} aria-label="Day" type="date" className="px-3 py-2 rounded-lg border border-line-strong bg-surface text-sm" />
        : <text data-testid="lab-orphan-gone" className="text-sm text-muted">The field is gone.</text>}
      <Button testId="lab-orphan-toggle" size="sm" variant="secondary" onClick={() => shown.set(!shown.value)}>
        {shown.value ? 'Remove the field' : 'Bring it back'}
      </Button>
    </Section>
  );
}

function Reveal() {
  return (
    <Section title="Reveal" testId="lab-reveal">
      <view className="flex-row gap-2">
        <Button testId="lab-reveal-far" size="sm" variant="secondary" onClick={() => reveal('lab-reveal-row-25')}>Row 25</Button>
        <Button testId="lab-reveal-first" size="sm" variant="secondary" onClick={() => reveal('lab-reveal-row-0')}>Row 0</Button>
      </view>
      <scroll data-testid="lab-reveal-scroll" className="h-32 rounded-xl bg-surface-sunken">
        {Array.from({ length: 30 }, (_, i) => (
          <text key={`row-${i}`} id={`lab-reveal-row-${i}`} data-testid={`lab-reveal-row-${i}`} className="h-8 px-3 text-sm text-text">{`Row ${i}`}</text>
        ))}
      </scroll>
      {/* Room for the page to scroll under a vertical drag that starts on the scrub plot. */}
      <view className="h-96" />
    </Section>
  );
}

function Drag() {
  return <Section title="Drag" testId="lab-drag"><DragDemo /></Section>;
}

function DragChain() {
  return <Section title="Drag and scroll" testId="lab-chain"><DragChainDemo /></Section>;
}

function Detents() {
  return <Section title="Sheet detents" testId="lab-detents"><DetentsDemo /></Section>;
}

// Rows that swipe open to Archive and Delete behind them; a full swipe deletes. Screen readers and
// specs reach both as the row's own actions (`aria-actions`). The strip is exactly as wide as the
// reveal (two 72 px actions = 144), so an open row shows no gap past its last action.

function SwipeRows() {
  return <Section title="Swipe rows" testId="lab-swipe"><SwipeRowsDemo /></Section>;
}

function Pager() {
  return <Section title="Pager" testId="lab-pager"><PagerDemo /></Section>;
}

// The kit pager: dots under the pages, tap one to jump; the pages read "n of m" as a list.
function KitPager() {
  const page = useLocal('kit-pager-page', 0);
  const pages = ['Plan', 'Pay', 'Done'];
  return (
    <Section title="Kit pager" testId="lab-kit-pager">
      <KitPagerView bind={page} aria-label="Checkout steps" data-testid="lab-kit-pager-view" className="w-72">
        {pages.map(name => (
          <view key={name} className="h-28 items-center justify-center rounded-xl bg-surface-sunken">
            <text className="text-lg font-semibold">{name}</text>
          </view>
        ))}
      </KitPagerView>
      <text data-testid="lab-kit-pager-state" className="text-xs text-muted tabular-nums">{`page=${page.value}`}</text>
    </Section>
  );
}
