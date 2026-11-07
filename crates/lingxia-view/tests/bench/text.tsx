// perf-lab's Text scene without the showcase: page chrome in components and one paragraph text
// whose content grows and whose width changes every Logic tick.
import type { View } from '@lingxia/view';

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

const PARAGRAPH = 'LingXia 灵匣 lays out mixed scripts: 文字排版需要处理中文、English words, emoji 🚀✨🎉 and numbers 12345 together. ';

export function render({ data }: View<Record<string, never>, { tick?: number }>) {
  const tick = data.tick ?? 0;
  const typed = 'typing… 輸入中 ⌨️ '.repeat(Math.floor(tick / 6) % 20);
  const width = 200 + ((tick * 3) % 160);
  return (
    <page className="bg-canvas p-4 gap-3">
      <Tabs active="Text" />
      <Section title="Text" description="paragraphs: 4">
        <Counts />
        <text className="text-xs text-muted tabular-nums">{`Logic ticks seen: ${tick}`}</text>
        <column className="h-80 overflow-hidden">
          <text className="text-sm" style={{ width }}>{PARAGRAPH.repeat(4) + typed}</text>
        </column>
      </Section>
      <Section title="Stall" description="Runs View code past its time budget.">
        <button className="px-2 py-1 rounded-md bg-surface-sunken text-xs" onClick={() => {}}>Stall the View</button>
      </Section>
    </page>
  );
}
