import { useLocal, type ReadonlyData } from '@lingxia/ui';
import type { ChartData } from './stream-types';
import { chartScale } from './chart-scale';
import { chartPercentages } from './chart-percent';
import { chartEmphasisColor } from './chart-emphasis';

const palette = ['#3B82F6', '#8B5CF6', '#10B981', '#F59E0B', '#EF4444', '#06B6D4'];

export function Chart({ data }: { data: ReadonlyData<ChartData> }) {
  const selected = useLocal('selected-series', -1);
  const hidden = useLocal<number[]>('hidden-series', []);
  const pie = data.kind === 'pie';
  const visible = data.series.map((_, index) => index).filter(index => !pie || !hidden.value.includes(index));
  const values = visible.map(index => data.series[index].value);
  const toggle = (index: number) => {
    hidden.set(hidden.value.includes(index) ? hidden.value.filter(value => value !== index) : [...hidden.value, index]);
    selected.set(-1);
  };
  const percentages = pie ? chartPercentages(values) : [];
  const scale = chartScale(values);
  const ticks = scale.ticks;
  const tickY = (value: number) => 12 + (scale.max - value) / (scale.max - scale.min) * 120;
  const axisNumber = (value: number) => String(Number(value.toPrecision(3)));
  const colorSet = visible.map(index => {
    const color = pie ? palette[index % palette.length] : palette[0];
    return index === selected.value && data.kind !== 'line' ? chartEmphasisColor(color) : color;
  });
  const item = data.series[selected.value];
  return <column className="pt-3">
    <column testId={`chart-${data.kind}`} className="bg-surface-50 border border-line-200 rounded-2xl shadow-sm">
      <text testId="chart-title" className="text-[10px] font-semibold tracking-widest text-gray-400 px-3.5 pt-3 pb-0.5">{data.title.toUpperCase()}</text>
      <row className={pie ? "" : "px-2"} style={{ gap: pie ? 0 : 4 }}>
        {!pie ? <stack testId="chart-y-axis" className="w-7" style={{ height: 144 }}>
          {ticks.map((value, index) => <text key={`tick-${index}`} testId={`chart-tick-${index}`} className="text-[11px] leading-6 text-right text-gray-500" style={{ height: 24, offset: { y: tickY(value) - 12 } }}>{axisNumber(value)}</text>)}
        </stack> : null}
        <stack className="grow basis-0 min-w-0" style={{ height: pie ? 210 : 144 }}>
          {!pie ? ticks.map((value, index) => <row key={`grid-${index}`} className="px-1" style={{ height: 1, offset: { y: tickY(value) } }}><vector fit="fill" viewBox={[0, 0, 1, 1]} className={value === 0 ? "grow basis-0 h-px text-line-200" : "grow basis-0 h-px text-line-100"} paths={[{ d: "M0 .5H1", fill: "none", stroke: "currentColor", strokeWidth: 1, strokeDasharray: value === 0 ? [] : [4, 2], vectorEffect: "non-scaling-stroke" }]} /></row>) : null}
          <plot testId={`chart-plot-${data.kind}`} kind={data.kind} values={values.length ? values : [0]} colors={colorSet.length ? colorSet : ['#00000000']}
            domainMin={pie ? undefined : scale.min} domainMax={pie ? undefined : scale.max}
            pieLayout={pie ? { center: [0.5, 0.44], innerRadius: 0.4, outerRadius: 0.68, emphasisScale: 5 } : undefined}
            smooth={data.kind === 'line'} area={data.kind === 'line'}
            markerEmphasisScale={data.kind === 'line' ? 1.1 : undefined}
            selectedIndex={visible.indexOf(selected.value)} onSelect={index => selected.set(visible[index] ?? -1)}
            accessibilityLabel={`${data.title}, ${data.kind} chart. ${data.series.map((item, index) => `${item.label}: ${item.value}${pie && hidden.value.includes(index) ? " (hidden)" : ""}`).join(', ')}`}
            style={{ height: pie ? 210 : 128, offset: { y: pie ? 0 : 8 } }} />
          {!pie && item ? <grid columns={data.series.length} className="px-1" style={{ height: 120, offset: { y: 12 } }}>
            {data.series.map((_, index) => <column key={`pointer-${index}`} className={selected.value === index ? 'h-full bg-[#81828833]' : 'h-full'} />)}
          </grid> : null}
          {pie ? <column className="h-full justify-end"><row testId="chart-legend" className="flex-wrap justify-center px-2 pb-1 gap-3">
        {data.series.map((item, index) => <button key={`${index}-${item.label}`} testId={`chart-series-${index}`} accessibilityLabel={`${item.label}: ${item.value}, ${hidden.value.includes(index) ? "hidden, show series" : "visible, hide series"}`} onClick={() => toggle(index)} className="max-w-full min-w-0 bg-surface-50 text-gray-500 py-0 px-0 rounded items-center justify-center">
          <row className="items-center gap-1.5"><column className="w-2 h-2 rounded-full" style={{ background: hidden.value.includes(index) ? "#9CA3AF" : palette[index % palette.length] }} /><text className={hidden.value.includes(index) ? "shrink min-w-0 text-[11px] leading-none text-gray-400" : "shrink min-w-0 text-[11px] leading-none"}>{item.label}</text></row>
        </button>)}
      </row></column> : null}
          {item ? <row className="justify-end items-start px-1 pt-1">
            <text testId="chart-tooltip" className="shrink min-w-0 px-2 py-1 text-xs text-gray-700 bg-surface border border-line-200 rounded-lg shadow-sm">{`${item.label}: ${pie ? `${percentages[visible.indexOf(selected.value)] ?? 0}%` : item.value}`}</text>
          </row> : null}
        </stack>
      </row>
      {!pie ? <row className="px-2 gap-1 pb-2">
        <spacer className="w-7" />
        <grid testId="chart-x-axis" columns={data.series.length} className="grow basis-0 min-w-0 px-1">
          {data.series.map((item, index) => <column key={`${index}-${item.label}`} className="items-center"><text testId={`chart-label-${index}`} lines={1} className="text-[11px] text-gray-500">{item.label}</text></column>)}
        </grid>
      </row> : null}
    </column>
  </column>;
}
