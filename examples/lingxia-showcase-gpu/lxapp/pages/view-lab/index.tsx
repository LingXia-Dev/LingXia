import { useLocal, useView, type VectorPath } from '@lingxia/ui';
import '../../theme/index.css';
import { Button, Icon, Page, Section } from '../../shared/ui';

// GPU-only renderer fixtures: one section per View capability, addressed by testId.
// One group renders at a time: a whole-lab render exceeds the per-render budget on slower phones.
const groups = {
  paint: { label: 'Paint', sections: [Grid, Gradients, Vectors, Borders, Transforms] },
  layout: { label: 'Layout', sections: [Display, Sizing, Direction, Structure, StickyHeaders] },
  text: { label: 'Text', sections: [TextLayout, RichText, Selection] },
  input: { label: 'Input', sections: [Accessibility, Inputs] },
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
      {groups[group.value].sections.map(Part => <Part key={Part.name} />)}
    </Page>
  );
}

function Gradients() {
  return (
    <Section title="Gradients" testId="lab-gradients">
      <row className="gap-3">
        <view testId="lab-gradient-radial" className="h-16 grow basis-0 rounded-xl bg-radial from-primary to-transparent" />
        <view testId="lab-gradient-conic" className="h-16 grow basis-0 rounded-xl bg-conic-90 from-primary via-error to-primary" />
        <view testId="lab-gradient-arbitrary" className="h-16 grow basis-0 rounded-xl bg-[radial-gradient(circle_at_top,#ffffff,#1677ff_70%)]" />
      </row>
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
        <vector testId="lab-vector-gradient" className="w-16 h-16" paths={[{ d: star, fill: 'url(#gold)', stroke: 'url(#edge)', strokeWidth: 0.75 }]}
          gradients={{
            gold: { type: 'radial', fx: 0.35, fy: 0.3, stops: [{ offset: 0, color: '#fff3b0' }, { offset: 1, color: '#f59e0b' }] },
            edge: { type: 'linear', x2: 0, y2: 1, stops: [{ offset: 0, color: '#b45309' }, { offset: 1, color: '#78350f', opacity: 0.6 }] },
          }} />
        <vector testId="lab-vector-strokes" className="w-16 h-16 text-foreground" paths={strokes} />
        <vector testId="lab-vector-conic" className="w-16 h-16" paths={[{ d: 'M12 2a10 10 0 1 0 0.01 0Z', fill: 'url(#wheel)' }]}
          gradients={{ wheel: { type: 'conic', stops: [{ offset: 0, color: '#ef4444' }, { offset: 0.33, color: '#22c55e' }, { offset: 0.66, color: '#3b82f6' }, { offset: 1, color: '#ef4444' }] } }} />
        <vector testId="lab-vector-transform" className="w-16 h-16 text-primary"
          paths={[0, 45, 90, 135].map(angle => ({ d: 'M12 4h1v16h-1z', transform: `rotate(${angle} 12.5 12)` }))} />
      </row>
    </Section>
  );
}

function Borders() {
  return (
    <Section title="Borders" testId="lab-borders">
      <row className="gap-3">
        <view testId="lab-border-dashed" className="h-12 grow basis-0 rounded-xl border-2 border-dashed border-primary" />
        <view testId="lab-border-dotted" className="h-12 grow basis-0 rounded-full border-4 border-dotted border-error" />
        <view testId="lab-border-edges" className="h-12 grow basis-0 border-t-2 border-l-4 border-dashed border-foreground" />
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
        <text testId="lab-display-hidden" className={shown.value ? 'text-sm' : 'hidden'}>Toggled by hidden</text>
      </row>
      <row className="block gap-1">
        <text className="text-xs text-muted-foreground">block: a column that stretches</text>
        <view className="h-2 rounded bg-primary" />
      </row>
      <row className="h-16 items-center gap-2 bg-muted rounded-lg px-2">
        <view testId="lab-display-inline" className="inline-flex px-2 py-1 rounded bg-card"><text className="text-xs">inline-flex</text></view>
        <view className="h-[8dvh] w-4 rounded bg-primary" />
      </row>
      <view testId="lab-display-fixed" className="fixed bottom-4 right-4 px-3 py-1 rounded-full bg-primary"><text className="text-xs text-primary-foreground">fixed</text></view>
    </Section>
  );
}

function Transforms() {
  return (
    <Section title="Transforms" testId="lab-transforms">
      <row className="gap-4 items-center">
        <Icon name="chevron" className="w-6 h-6 text-foreground rotate-90" />
        <Icon name="send" className="w-6 h-6 text-primary -scale-x-100" />
        <view className="relative w-16 h-10 rounded-lg bg-muted">
          <view testId="lab-transform-badge" className="absolute top-0 right-0 w-4 h-4 rounded-full bg-error translate-x-1/2 -translate-y-1/2" />
        </view>
        <view testId="lab-transform-card" className="w-12 h-12 rounded-lg bg-primary origin-bottom-left rotate-12 scale-90" />
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
        <text testId="lab-text-pre-line" className="grow basis-0 text-sm whitespace-pre-line">{'First line\n   second line keeps   one space'}</text>
      </row>
      <text testId="lab-text-pre-wrap" className="p-2 rounded bg-muted text-xs font-mono whitespace-pre-wrap">{'{\n  "indent": true\n}'}</text>
      <text testId="lab-text-break-all" className="text-xs break-all">https://example.com/a/very/long/path/without/any/spaces/to/break/at</text>
    </Section>
  );
}

function Sizing() {
  return (
    <Section title="Sizing" testId="lab-sizing">
      <row className="gap-3 items-start">
        <view testId="lab-aspect-video" className="grow basis-0 aspect-video rounded-lg bg-primary" />
        <view testId="lab-aspect-square" className="w-16 aspect-square rounded-lg text-error bg-current" />
      </row>
      <text testId="lab-max-ch" className="max-w-[24ch] text-sm select-none">This paragraph wraps at about twenty-four zeros of its own font.</text>
      <scroll testId="lab-overscroll" className="h-24 rounded-lg border border-border overscroll-contain">
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
      <view testId="lab-grid-auto-fit" className="grid grid-cols-[repeat(auto-fit,minmax(6rem,1fr))] gap-2">
        {tiles.map(tile => <view key={tile} className="h-14 rounded-lg bg-muted place-items-center grid"><text className="text-xs">{tile}</text></view>)}
      </view>
      <view testId="lab-grid-spans" className="grid grid-cols-3 grid-rows-[auto_auto] gap-2">
        <view className="col-span-2 h-12 rounded-lg bg-primary" />
        <view className="row-span-2 rounded-lg bg-error" />
        <view className="h-12 rounded-lg bg-muted" />
        <view className="h-12 rounded-lg bg-muted order-first" />
      </view>
    </Section>
  );
}

function Direction() {
  const { env } = useView();
  return (
    <Section title={`Direction (env: ${env.direction})`} testId="lab-direction">
      {(['ltr', 'rtl'] as const).map(dir => (
        <row key={dir} dir={dir} testId={`lab-direction-${dir}`} className="items-center gap-2 ps-3 pe-1 py-2 rounded-s-xl border-s-4 border-primary bg-muted">
          <Icon name="chevron" className="w-5 h-5 text-foreground rtl:-scale-x-100" />
          <text className="grow basis-0 text-sm text-start">{dir === 'rtl' ? 'مرحبا بالعالم' : 'Hello, world'}</text>
          <text className="text-xs text-muted-foreground ms-auto">{dir}</text>
        </row>
      ))}
    </Section>
  );
}

function RichText() {
  const presses = useLocal('presses', 0);
  return (
    <Section title="Rich text" testId="lab-rich-text">
      <text testId="lab-rich-paragraph" className="text-sm leading-relaxed">
        Plans renew <span className="font-semibold text-foreground">monthly</span> unless you{' '}
        <span className="bg-muted font-mono">cancel</span> first. Read the{' '}
        <link testId="lab-rich-link" onPress={() => presses.set(presses.value + 1)} className="active:text-foreground">terms of service and <span className="font-semibold">privacy policy</span></link>
        {' '}before you <span className="italic line-through text-muted-foreground">sign</span> agree.
      </text>
      <text testId="lab-rich-presses" className="text-xs text-muted-foreground tabular-nums">Link pressed {presses.value} times</text>
    </Section>
  );
}

function Accessibility() {
  const saves = useLocal('saves', 0);
  return (
    <Section title="Accessibility" testId="lab-accessibility">
      <text testId="lab-a11y-heading" role="heading" headingLevel={3} className="text-base font-semibold">Monthly bills</text>
      <column testId="lab-a11y-list" role="list" accessibilityLabel="Bills" className="gap-1">
        {['Water', 'Power', 'Internet'].map(bill => (
          <row key={bill} role="listitem" className="justify-between">
            <text className="text-sm">{bill}</text>
            <vector accessibilityHidden className="w-4 h-4 text-muted-foreground" paths={[{ d: 'M9 18l6-6-6-6', fill: 'none', stroke: 'currentColor', strokeWidth: 1.5 }]} />
          </row>
        ))}
      </column>
      <row className="items-center gap-3">
        <Button testId="lab-a11y-save" onClick={() => saves.set(saves.value + 1)}>Save</Button>
        <text testId="lab-a11y-status" role="status" className="text-sm text-muted-foreground">{saves.value ? `Saved ${saves.value} times` : 'Not saved yet'}</text>
      </row>
    </Section>
  );
}

function Inputs() {
  const code = useLocal('code', '');
  const amount = useLocal('amount', '');
  const day = useLocal('day', '');
  const time = useLocal('time', '');
  const field = 'px-3 py-2 rounded-lg border border-input bg-card text-sm';
  return (
    <Section title="Inputs" testId="lab-inputs">
      <input testId="lab-input-otp" bind={code} accessibilityLabel="One-time code" keyboard="number" maxLength={6} autoComplete="one-time-code" enterKeyHint="done" placeholder="6-digit code" className={field} />
      <input testId="lab-input-amount" bind={amount} accessibilityLabel="Amount" keyboard="number" min={1} max={500} step={0.5} enterKeyHint="next" placeholder="1 to 500, steps of 0.5" className={field} />
      <row className="gap-2">
        <input testId="lab-input-date" bind={day} accessibilityLabel="Billing date" keyboard="date" min="2024-01-01" placeholder="Pick a date" className={`grow basis-0 ${field}`} />
        <input testId="lab-input-time" bind={time} accessibilityLabel="Reminder time" keyboard="time" placeholder="Pick a time" className={`grow basis-0 ${field}`} />
      </row>
      <input testId="lab-input-name" accessibilityLabel="Name" autoComplete="name" autoCapitalize="words" spellCheck={false} placeholder="Full name" className={field} />
      <text testId="lab-input-values" className="text-xs text-muted-foreground">{`code=${code.value} amount=${amount.value} date=${day.value} time=${time.value}`}</text>
    </Section>
  );
}

function Structure() {
  const rows = ['Starter', 'Plus', 'Pro', 'Max'];
  return (
    <Section title="Structural variants" testId="lab-structure">
      <column testId="lab-structure-rows" className="rounded-lg border border-border">
        {rows.map(row => (
          <text key={row} className="px-3 py-2 text-sm odd:bg-muted first:rounded-t-lg last:rounded-b-lg last:font-semibold">{row}</text>
        ))}
      </column>
      <row className="gap-2">
        <view testId="lab-structure-empty" className="h-8 grow basis-0 rounded-lg bg-error empty:bg-muted" />
        <view className="h-8 grow basis-0 rounded-lg bg-muted"><text className="only:text-xs only:p-2">only child</text></view>
      </row>
    </Section>
  );
}

function StickyHeaders() {
  const groups = { Today: ['Top up', 'Data pack'], Yesterday: ['Bill paid', 'Roaming on', 'SMS bundle'], 'Last week': ['Plan change', 'Refund'] };
  return (
    <Section title="Sticky headers" testId="lab-sticky">
      <scroll testId="lab-sticky-scroll" className="h-48 rounded-lg border border-border">
        {Object.entries(groups).map(([day, items]) => (
          <column key={day}>
            <text className="sticky top-0 px-3 py-1.5 text-xs font-semibold bg-muted">{day}</text>
            {items.map(item => <text key={item} className="px-3 py-3 text-sm border-b border-border">{item}</text>)}
          </column>
        ))}
      </scroll>
    </Section>
  );
}

function Selection() {
  return (
    <Section title="Selectable text" testId="lab-selection">
      <text testId="lab-select-text" className="text-sm select-text">Long-press a word to select it, then drag the handles. Order reference: LX-2024-88412.</text>
      <text testId="lab-select-all" className="font-mono text-sm px-2 py-1 rounded bg-muted select-all">8F3A-77C2-91D0</text>
      <view className="select-text"><text testId="lab-select-inherited" className="text-xs text-muted-foreground">Selectable by inheritance.</text></view>
    </Section>
  );
}
