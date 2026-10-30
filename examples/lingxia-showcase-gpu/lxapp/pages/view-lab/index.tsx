import { Details, Fragment, List, Summary, useLocal, useView, type VectorPath } from '@lingxia/ui';
import '../../theme/index.css';
import { Button, Icon, Page, Section } from '../../shared/ui';

// GPU-only renderer fixtures: one section per View capability, addressed by testId.
// One group renders at a time: a whole-lab render exceeds the per-render budget on slower phones.
const groups = {
  paint: { label: 'Paint', sections: [Grid, Gradients, Vectors, Borders, Transforms] },
  layout: { label: 'Layout', sections: [Display, Sizing, Direction, Structure, StickyHeaders, Fragments, Disclosure] },
  text: { label: 'Text', sections: [TextLayout, RichText, Selection] },
  input: { label: 'Input', sections: [Accessibility, Inputs, Controls, Focus, NestedControls, StateVariants] },
  overlay: { label: 'Overlay', sections: [Overlays] },
};
type Group = keyof typeof groups;

export function render() {
  const group = useLocal<Group>('group', 'paint');
  return (
    <Page testId="view-lab" title="View lab">
      <row className="gap-2">
        {(Object.keys(groups) as Group[]).map(key => (
          <Button key={key} testId={`lab-tab-${key}`} size="sm" variant={group.value === key ? 'primary' : 'secondary'} onClick={() => group.set(key)}>
            {groups[key].label}
          </Button>
        ))}
      </row>
      {groups[group.value].sections.map((Part, index) => <Part key={`${group.value}-${index}`} />)}
    </Page>
  );
}

function Gradients() {
  return (
    <Section title="Gradients" testId="lab-gradients">
      <row className="gap-3">
        <view data-testid="lab-gradient-radial" className="h-16 grow basis-0 rounded-xl bg-radial from-accent to-transparent" />
        <view data-testid="lab-gradient-conic" className="h-16 grow basis-0 rounded-xl bg-conic-90 from-accent via-danger to-accent" />
        <view data-testid="lab-gradient-arbitrary" className="h-16 grow basis-0 rounded-xl bg-radial-[at_top] from-surface to-accent to-70%" />
      </row>
      <column data-testid="lab-gradient-mask" className="h-16 rounded-xl bg-accent mask-b-from-20% mask-r-to-90% p-3">
        <text className="text-sm text-on-accent">Fades out downward and to the right</text>
      </column>
      <column data-testid="lab-gradient-halo" className="h-20 items-center justify-center">
        <view className="absolute w-16 h-16 rounded-full bg-accent/40 blur-xl" />
        <view className="w-10 h-10 rounded-full bg-accent" />
      </column>
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
      <row className="gap-4 items-center">
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
      </row>
    </Section>
  );
}

function Borders() {
  return (
    <Section title="Borders" testId="lab-borders">
      <row className="gap-3">
        <view data-testid="lab-border-dashed" className="h-12 grow basis-0 rounded-xl border-2 border-dashed border-accent" />
        <view data-testid="lab-border-dotted" className="h-12 grow basis-0 rounded-full border-4 border-dotted border-danger" />
        <view data-testid="lab-border-edges" className="h-12 grow basis-0 border-t-2 border-l-4 border-dashed border-text" />
      </row>
    </Section>
  );
}

function Display() {
  const shown = useLocal('shown', true);
  return (
    <Section title="Display" testId="lab-display">
      <row className="gap-2 items-center">
        <Button testId="lab-display-toggle" onClick={() => shown.set(!shown.value)}>{shown.value ? 'Hide' : 'Show'}</Button>
        <text data-testid="lab-display-hidden" className={shown.value ? 'text-sm' : 'hidden'}>Toggled by hidden</text>
      </row>
      <row className="block gap-1">
        <text className="text-xs text-muted">block: a column that stretches</text>
        <view className="h-2 rounded bg-accent" />
      </row>
      <row className="h-16 items-center gap-2 bg-surface-sunken rounded-lg px-2">
        <view data-testid="lab-display-inline" className="inline-flex px-2 py-1 rounded bg-surface"><text className="text-xs">inline-flex</text></view>
        <view className="h-[8dvh] w-4 rounded bg-accent" />
      </row>
      <view data-testid="lab-display-fixed" className="fixed bottom-4 right-4 px-3 py-1 rounded-full bg-accent"><text className="text-xs text-on-accent">fixed</text></view>
    </Section>
  );
}

function Transforms() {
  return (
    <Section title="Transforms" testId="lab-transforms">
      <row className="gap-4 items-center">
        <Icon name="chevron" className="w-6 h-6 text-text rotate-90" />
        <Icon name="send" className="w-6 h-6 text-accent -scale-x-100" />
        <view className="relative w-16 h-10 rounded-lg bg-surface-sunken">
          <view data-testid="lab-transform-badge" className="absolute top-0 right-0 w-4 h-4 rounded-full bg-danger translate-x-1/2 -translate-y-1/2" />
        </view>
        <view data-testid="lab-transform-card" className="w-12 h-12 rounded-lg bg-accent origin-bottom-left rotate-12 scale-90" />
      </row>
    </Section>
  );
}

function TextLayout() {
  return (
    <Section title="Text" testId="lab-text">
      <row className="gap-6">
        <column className="items-end">
          {['1,111.10', '808.08', '10.00'].map(value => <text key={value} className="text-sm tabular-nums slashed-zero">{value}</text>)}
        </column>
        <text data-testid="lab-text-pre-line" className="grow basis-0 text-sm whitespace-pre-line">{'First line\n   second line keeps   one space'}</text>
      </row>
      <text data-testid="lab-text-pre-wrap" className="p-2 rounded bg-surface-sunken text-xs font-mono whitespace-pre-wrap">{'{\n  "indent": true\n}'}</text>
      <text data-testid="lab-text-break-all" className="text-xs break-all">https://example.com/a/very/long/path/without/any/spaces/to/break/at</text>
    </Section>
  );
}

function Sizing() {
  return (
    <Section title="Sizing" testId="lab-sizing">
      <row className="gap-3 items-start">
        <view data-testid="lab-aspect-video" className="grow basis-0 aspect-video rounded-lg bg-accent" />
        <view data-testid="lab-aspect-square" className="w-16 aspect-square rounded-lg text-danger bg-current" />
      </row>
      <text data-testid="lab-max-ch" className="max-w-[24ch] text-sm select-none">This paragraph wraps at about twenty-four zeros of its own font.</text>
      <scroll data-testid="lab-overscroll" className="h-24 rounded-lg border border-line overscroll-contain">
        <column className="p-3 gap-2">
          {Array.from({ length: 8 }, (_, i) => <text key={String(i)} className="text-xs">Contained row {i + 1}</text>)}
        </column>
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
        <row key={dir} dir={dir} data-testid={`lab-direction-${dir}`} className="items-center gap-2 ps-3 pe-1 py-2 rounded-s-xl border-s-4 border-accent bg-surface-sunken">
          <Icon name="chevron" className="w-5 h-5 text-text rtl:-scale-x-100" />
          <text className="grow basis-0 text-sm text-start">{dir === 'rtl' ? 'مرحبا بالعالم' : 'Hello, world'}</text>
          <text className="text-xs text-muted ms-auto">{dir}</text>
        </row>
      ))}
    </Section>
  );
}

function RichText() {
  const presses = useLocal('presses', 0);
  return (
    <Section title="Rich text" testId="lab-rich-text">
      <text data-testid="lab-rich-paragraph" className="text-sm leading-relaxed">
        Plans renew <span className="font-semibold text-text">monthly</span> unless you{' '}
        <span className="bg-surface-sunken font-mono">cancel</span> first. Read the{' '}
        <link data-testid="lab-rich-link" onClick={() => presses.set(presses.value + 1)} className="active:text-text">terms of service and <span className="font-semibold">privacy policy</span></link>
        {' '}before you <span className="italic line-through text-muted">sign</span> agree.
      </text>
      <text data-testid="lab-rich-presses" className="text-xs text-muted tabular-nums">Link pressed {presses.value} times</text>
    </Section>
  );
}

function Accessibility() {
  const saves = useLocal('saves', 0);
  return (
    <Section title="Accessibility" testId="lab-accessibility">
      <text data-testid="lab-a11y-heading" role="heading" headingLevel={3} className="text-base font-semibold">Monthly bills</text>
      <column data-testid="lab-a11y-list" role="list" aria-label="Bills" className="gap-1">
        {['Water', 'Power', 'Internet'].map(bill => (
          <row key={bill} role="listitem" className="justify-between">
            <text className="text-sm">{bill}</text>
            <vector aria-hidden className="w-4 h-4 text-muted" paths={[{ d: 'M9 18l6-6-6-6', fill: 'none', stroke: 'currentColor', strokeWidth: 1.5 }]} />
          </row>
        ))}
      </column>
      <row className="items-center gap-3">
        <Button testId="lab-a11y-save" onClick={() => saves.set(saves.value + 1)}>Save</Button>
        <text data-testid="lab-a11y-status" role="status" className="text-sm text-muted">{saves.value ? `Saved ${saves.value} times` : 'Not saved yet'}</text>
      </row>
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
      <row className="gap-2">
        <input data-testid="lab-input-date" bind={day} aria-label="Billing date" type="date" min="2024-01-01" placeholder="Pick a date" className={`grow basis-0 ${field}`} />
        <input data-testid="lab-input-time" bind={time} aria-label="Reminder time" type="time" placeholder="Pick a time" className={`grow basis-0 ${field}`} />
      </row>
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
      <column data-testid="lab-structure-rows" className="rounded-lg border border-line">
        {rows.map(row => (
          <text key={row} className="px-3 py-2 text-sm odd:bg-surface-sunken first:rounded-t-lg last:rounded-b-lg last:font-semibold">{row}</text>
        ))}
      </column>
      <row className="gap-2">
        <view data-testid="lab-structure-empty" className="h-8 grow basis-0 rounded-lg bg-danger empty:bg-surface-sunken" />
        <view className="h-8 grow basis-0 rounded-lg bg-surface-sunken"><text className="only:text-xs only:p-2">only child</text></view>
      </row>
    </Section>
  );
}

function StickyHeaders() {
  const groups = { Today: ['Top up', 'Data pack'], Yesterday: ['Bill paid', 'Roaming on', 'SMS bundle'], 'Last week': ['Plan change', 'Refund'] };
  return (
    <Section title="Sticky headers" testId="lab-sticky">
      <scroll data-testid="lab-sticky-scroll" className="h-48 rounded-lg border border-line">
        {Object.entries(groups).map(([day, items]) => (
          <column key={day}>
            <text className="sticky top-0 px-3 py-1.5 text-xs font-semibold bg-surface-sunken">{day}</text>
            {items.map(item => <text key={item} className="px-3 py-3 text-sm border-b border-line">{item}</text>)}
          </column>
        ))}
      </scroll>
    </Section>
  );
}

function Selection() {
  return (
    <Section title="Selectable text" testId="lab-selection">
      <text data-testid="lab-select-text" className="text-sm select-text">Long-press a word to select it, then drag the handles. Order reference: LX-2024-88412.</text>
      <text data-testid="lab-select-all" className="font-mono text-sm px-2 py-1 rounded bg-surface-sunken select-all">8F3A-77C2-91D0</text>
      <view className="select-text"><text data-testid="lab-select-inherited" className="text-xs text-muted">Selectable by inheritance.</text></view>
    </Section>
  );
}

function Focus() {
  const open = useLocal('dialog-open', false);
  const choice = useLocal('dialog-choice', 'none');
  const close = (value: string) => { choice.set(value); open.set(false); };
  return (
    <Section title="Keyboard focus" testId="lab-focus">
      <text className="text-xs text-muted">With a keyboard: Tab to Open, press Enter; focus stays in the dialog and returns to Open when it closes.</text>
      <row className="gap-2 items-center">
        <Button testId="lab-focus-open" onClick={() => open.set(true)}>Open dialog</Button>
        <text data-testid="lab-focus-choice" className="text-sm">{`chosen: ${choice.value}`}</text>
      </row>
      <dialog open={open.value} onClose={() => close('escape')} data-testid="lab-focus-dialog" aria-label="Confirm payment" className="w-72 gap-3 p-4 rounded-xl border border-line bg-surface">
        <text role="heading" headingLevel={3} className="text-sm font-semibold">Confirm payment</text>
        <row className="gap-2">
          <Button testId="lab-focus-confirm" onClick={() => close('confirm')}>Confirm</Button>
          <Button testId="lab-focus-cancel" variant="secondary" onClick={() => close('cancel')}>Cancel</Button>
        </row>
      </dialog>
    </Section>
  );
}

// A pressable row holding its own controls: each press goes to the innermost control under it.
function NestedControls() {
  const opened = useLocal('opened', 0);
  const wifi = useLocal('wifi', true);
  const removed = useLocal('removed', 0);
  return (
    <Section title="Nested controls" testId="lab-nested">
      <button data-testid="lab-nested-row" onClick={() => opened.set(opened.value + 1)} className="rounded-xl bg-surface-sunken px-4 py-3">
        <row className="items-center gap-3">
          <text className="grow text-sm">Wi-Fi</text>
          <switch data-testid="lab-nested-switch" bind={wifi} aria-label="Wi-Fi" />
          <button data-testid="lab-nested-remove" onClick={() => removed.set(removed.value + 1)} className="rounded-lg px-2 py-1 text-sm text-danger">Remove</button>
        </row>
      </button>
      <text data-testid="lab-nested-state" className="text-xs text-muted tabular-nums">{`opened=${opened.value} wifi=${wifi.value} removed=${removed.value}`}</text>
    </Section>
  );
}

function Term({ word, meaning }: { word: string; meaning: string }) {
  return <><text className="text-sm font-semibold">{word}</text><text className="text-xs text-muted">{meaning}</text></>;
}

// Fragments add no box: the terms sit directly in the column, and a keyed group moves as one.
function Fragments() {
  const words = useLocal('fragment-words', [['GPU', 'draws the page'], ['View', 'describes it'], ['Logic', 'owns the data']]);
  const note = useLocal('fragment-note', false);
  return (
    <Section title="Fragments" testId="lab-fragments">
      <row className="gap-2">
        <Button testId="lab-fragments-rotate" size="sm" onClick={() => words.set([...words.value.slice(1), words.value[0]])}>Rotate</Button>
        <Button testId="lab-fragments-note" size="sm" onClick={() => note.set(!note.value)}>{note.value ? 'Hide note' : 'Show note'}</Button>
      </row>
      <column data-testid="lab-fragments-list" className="gap-1">
        {note.value && <text data-testid="lab-fragments-hint" className="text-xs">Each term is two texts in one fragment.</text>}
        {words.value.map(([word, meaning]) => <Fragment key={word}><Term word={word} meaning={meaning} /></Fragment>)}
      </column>
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
      <row data-testid="lab-variants-agree" className="items-center gap-3 rounded-xl px-4 py-3 bg-surface-sunken has-checked:bg-accent has-checked:text-on-accent">
        <checkbox data-testid="lab-variants-checkbox" bind={agreed} aria-label="Agree" />
        <text className="text-sm">{agreed.value ? 'Agreed' : 'Tap to agree'}</text>
      </row>
      <button data-testid="lab-variants-toggle" data-state={open.value ? 'open' : 'closed'} onClick={() => open.set(!open.value)}
        className="rounded-xl px-4 py-3 bg-surface-sunken data-[state=open]:bg-accent data-[state=open]:text-on-accent data-[state=open]:py-5">
        {open.value ? 'Open (taller)' : 'Closed'}
      </button>
      <column data-testid="lab-variants-field" className="rounded-xl p-3 gap-1 border border-line focus-within:border-accent focus-within:bg-surface-sunken">
        <text className="text-xs text-muted">Note</text>
        <input data-testid="lab-variants-input" bind={note} placeholder="Focus me" aria-label="Note" />
      </column>
    </Section>
  );
}

const rows = Array.from({ length: 40 }, (_, index) => ({ id: `row-${index + 1}`, label: `Row ${index + 1}` }));

// Every control kind a spec drives: radio group, slider, multi-line editor, search field and a keyed list.
function Controls() {
  const size = useLocal('controls-size', 'm');
  const volume = useLocal('controls-volume', 40);
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
      <textarea data-testid="lab-controls-notes" bind={notes} rows={3} maxRows={6} aria-label="Notes" placeholder="Several lines" className={field} />
      <input type="search" data-testid="lab-controls-search" bind={query} aria-label="Filter rows" placeholder="Filter rows" className={field} />
      <List data-testid="lab-controls-list" className="h-48 rounded-lg border border-line"
        items={rows.filter(row => row.label.toLowerCase().includes(query.value.toLowerCase()))} getKey={row => row.id}
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
      <row className="gap-2 items-center flex-wrap">
        <Button testId="lab-overlay-confirm" onClick={() => confirm.set(true)}>Dialog</Button>
        <Button testId="lab-overlay-sheet" variant="secondary" onClick={() => sheet.set(true)}>Sheet</Button>
        <button popoverTarget="lab-menu" data-testid="lab-overlay-more" aria-label="More" className="ms-auto rounded-full px-3 py-1.5 bg-surface-sunken text-text">⋯</button>
      </row>
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
        <row className="gap-2 justify-end">
          <Button testId="lab-dialog-cancel" variant="secondary" onClick={() => { act('cancel'); confirm.set(false); }}>Cancel</Button>
          <Button testId="lab-dialog-delete" onClick={() => { act('delete'); confirm.set(false); }}>Delete</Button>
        </row>
      </dialog>
      <dialog open={sheet.value} onClose={() => sheet.set(false)} closedby="any" aria-label="Filters" data-testid="lab-sheet"
        style={{ paddingBottom: 16 + safeBottom }}
        className="mt-auto mb-0 w-full p-4 gap-3 rounded-t-3xl bg-surface-raised transition-transform duration-300 ease-out starting:translate-y-full exit:translate-y-full">
        <spacer className="self-center w-10 h-1 rounded-full bg-line-strong" />
        <text role="heading" headingLevel={3} className="text-base font-semibold">Filters</text>
        <scroll data-testid="lab-sheet-scroll" className="max-h-48">
          {Array.from({ length: 12 }, (_, i) => <text key={`f${i}`} className="py-2 text-sm">{`Filter ${i + 1}`}</text>)}
        </scroll>
        <Button testId="lab-sheet-apply" onClick={() => { act('apply'); sheet.set(false); }}>Apply</Button>
      </dialog>
    </Section>
  );
}
