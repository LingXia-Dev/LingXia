// perf-lab's Data scene without the showcase: page chrome in components, a tick text and a
// 1000-row List whose anchor moves two rows per Logic tick, so every tick is a full View render.
import { List, type View } from '@lingxia/view';

const scenes = ['List', 'Data', 'Animation', 'Fill', 'Text', 'Touch', 'Transition', 'Images'];
// Frozen, as perf-lab's rows are: a List reuses committed rows only over frozen items.
const rows = Object.freeze(Array.from({ length: 1000 }, (_, i) => i));

function Tabs({ active }: { active: string }) {
  return (
    <row className="gap-2 flex-wrap">
      {scenes.map(name => (
        <button key={name} className={`px-3 py-1 rounded-lg text-sm ${name === active ? 'bg-accent text-on-accent' : 'bg-surface'}`}
          onClick={() => {}}>{name}</button>
      ))}
    </row>
  );
}

function Section({ title, description, children }: { title: string; description: string; children?: unknown }) {
  return (
    <column className="gap-2 p-3 rounded-xl bg-surface">
      <text className="text-base font-semibold">{title}</text>
      <text className="text-xs text-muted">{description}</text>
      {children}
    </column>
  );
}

function Counts() {
  return (
    <row className="gap-2">
      {[100, 1000, 5000].map(value => (
        <button key={String(value)} className="px-2 py-1 rounded-md bg-surface-sunken text-xs" onClick={() => {}}>{String(value)}</button>
      ))}
    </row>
  );
}

export function render({ data }: View<Record<string, never>, { tick?: number }>) {
  const tick = data.tick ?? 0;
  const step = tick * 2;
  return (
    <page className="bg-canvas p-4 gap-3">
      <Tabs active="Data" />
      <Section title="Data" description="Logic moves the list anchor two rows per tick">
        <Counts />
        <text className="text-xs text-muted tabular-nums">{`Logic ticks seen: ${tick}`}</text>
        <List className="h-96 rounded-lg border border-line" items={rows} getKey={i => String(i)}
          anchor={{ index: step % rows.length, align: 'start', seq: step }}
          renderItem={i => (
            <row className="items-center gap-3 px-3 py-2 border-b border-line">
              <view className="w-10 h-10 rounded-full bg-accent" />
              <column className="grow basis-0 min-w-0">
                <text className="text-sm font-medium truncate">Row {i + 1}</text>
                <text className="text-xs text-muted truncate">A second line of text to lay out in every row</text>
              </column>
              <text className="px-2 rounded-full bg-accent text-on-accent text-xs tabular-nums">{String(i % 100)}</text>
            </row>
          )} />
      </Section>
      <Section title="Stall" description="Runs View code past its time budget.">
        <button className="px-2 py-1 rounded-md bg-surface-sunken text-xs" onClick={() => {}}>Stall the View</button>
      </Section>
    </page>
  );
}
