// perf-lab's Images scene without the showcase: page chrome in components and a frozen 600-row
// List of three images per row whose anchor moves two rows per Logic tick.
import { List, type View } from '@lingxia/view';

const scenes = ['List', 'Data', 'Animation', 'Fill', 'Text', 'Touch', 'Transition', 'Images'];

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

const rows = Object.freeze(Array.from({ length: 600 }, (_, i) => i));
const size = 96;
const image = (index: number) => `public/perf/img-${String(index % 16).padStart(2, '0')}.png`;

export function render({ data }: View<Record<string, never>, { tick?: number }>) {
  const tick = data.tick ?? 0;
  const step = tick * 2;
  return (
    <page className="bg-canvas p-4 gap-3">
      <Tabs active="Images" />
      <Section title="Images" description="image px: 96">
        <Counts />
        <text className="text-xs text-muted tabular-nums">{`Logic ticks seen: ${tick}`}</text>
        <List className="h-96 rounded-lg border border-line" items={rows} getKey={i => String(i)}
          anchor={{ index: step % rows.length, align: 'start', seq: step }}
          renderItem={i => (
            <row className="items-end gap-2 px-3 py-2 border-b border-line">
              {[1, 0.75, 0.5].map((factor, k) => (
                <image key={String(k)} src={image(i * 3 + k)} alt="" className="rounded-md"
                  style={{ width: Math.round(size * factor) + (i % 7), height: Math.round(size * factor) + (i % 7) }} />
              ))}
            </row>
          )} />
      </Section>
      <Section title="Stall" description="Runs View code past its time budget.">
        <button className="px-2 py-1 rounded-md bg-surface-sunken text-xs" onClick={() => {}}>Stall the View</button>
      </Section>
    </page>
  );
}
