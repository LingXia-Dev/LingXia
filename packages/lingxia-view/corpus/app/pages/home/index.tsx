import type { View } from '@lingxia/view';

const tiles = ['Speed', 'Devices', 'Family', 'Sharing', 'Security', 'Usage'];
const ring = 2 * Math.PI * 42;

export function render({ data }: View<object, { user: string; down: number; up: number; ping: number; history: number[] }>) {
  const progress = Math.min(1, data.down / 1000);
  const spark = data.history.map((value, index) => `${index === 0 ? 'M' : 'L'}${index * 10} ${40 - value * 0.4}`).join(' ');
  return (
    <page className="bg-canvas">
      <scroll className="grow">
        <column className="sticky top-0 z-10 px-4 py-3 bg-surface/90 backdrop-blur-xl border-b border-line">
          <text role="heading" className="text-lg font-semibold">Hello, {data.user}</text>
        </column>
        <column className="m-4 p-5 gap-3 rounded-2xl bg-linear-to-br from-accent to-chart-4 shadow-lg">
          <text className="text-xs uppercase tracking-widest text-on-accent/80">Network</text>
          <row className="items-center gap-4">
            <vector aria-hidden className="w-24 h-24 text-on-accent" viewBox={[0, 0, 100, 100]} paths={[
              { d: 'M50 8a42 42 0 1 1 0 84a42 42 0 1 1 0-84', fill: 'none', stroke: 'currentColor', strokeWidth: 8, strokeDasharray: [ring], strokeDashoffset: ring * (1 - progress), strokeLinecap: 'round', transform: 'rotate(-90 50 50)' },
            ]} />
            <column className="gap-1">
              <text className="text-3xl font-bold tabular-nums text-on-accent">{data.down}</text>
              <text className="text-sm text-on-accent/80 tabular-nums">↑ {data.up} Mbps · {data.ping} ms</text>
            </column>
          </row>
          <vector aria-hidden className="h-10 w-full text-on-accent" viewBox={[0, 0, 110, 40]} paths={[{ d: spark, fill: 'none', stroke: 'currentColor', strokeWidth: 2, vectorEffect: 'non-scaling-stroke' }]} />
        </column>
        <grid className="mx-4 grid-cols-3 gap-3">
          {tiles.map(tile => (
            <button key={tile} onClick={() => {}} className="h-24 p-3 rounded-xl bg-surface border border-line items-center justify-center active:bg-surface-sunken">
              <text className="text-sm font-medium">{tile}</text>
            </button>
          ))}
        </grid>
        <row className="m-4 p-4 gap-3 items-center rounded-xl bg-surface shadow-sm">
          <image src="public/avatar.png" alt="" className="w-12 h-12 rounded-full object-cover bg-surface-sunken" />
          <column className="grow gap-0.5 min-w-0">
            <text className="text-sm font-semibold truncate">A very long household name that should truncate gracefully</text>
            <text className="text-xs text-muted">3 people · 12 devices</text>
          </column>
        </row>
      </scroll>
    </page>
  );
}
