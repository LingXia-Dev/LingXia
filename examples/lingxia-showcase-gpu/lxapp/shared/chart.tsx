import { useLocal, type ReadonlyData } from '@lingxia/view';
import type { ChartData } from './stream-types';
import { chartScale } from './chart-scale';
import { chartPercentages } from './chart-percent';

const palette = ['chart-1', 'chart-2', 'chart-3', 'chart-4', 'chart-5'];
const plotHeight = 128,
  axisTop = 12,
  axisSpan = 120;

/** Bar, line and pie charts with a selectable series and legend toggles for pies. */
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
  const tickY = (value: number) => axisTop + ((scale.max - value) / (scale.max - scale.min)) * axisSpan;
  const axisNumber = (value: number) => String(Number(value.toPrecision(3)));
  const colors = visible.map(index => {
    const color = pie ? palette[index % palette.length] : palette[0];
    // A selection dims the other bars and slices rather than recolouring the chosen one.
    return selected.value < 0 || index === selected.value || data.kind === 'line' ? color : `${color}/60`;
  });
  const item = data.series[selected.value];
  const description = data.series
    .map((entry, index) => `${entry.label}: ${entry.value}${pie && hidden.value.includes(index) ? ' (hidden)' : ''}`)
    .join(', ');
  return (
    <column data-testid={`chart-${data.kind}`} className="mt-3 rounded-2xl border border-line bg-surface-sunken shadow-sm">
      <text data-testid="chart-title" className="px-3.5 pt-3 pb-0.5 text-[10px] font-semibold tracking-widest uppercase text-muted">
        {data.title}
      </text>
      <row className={pie ? '' : 'px-2 gap-1'}>
        {!pie && (
          <stack data-testid="chart-y-axis" className="w-7 h-36">
            {scale.ticks.map((value, index) => (
              <text
                key={`tick-${index}`}
                data-testid={`chart-tick-${index}`}
                className="absolute left-0 right-0 h-6 text-[11px] leading-6 text-right text-muted"
                style={{ top: tickY(value) - 12 }}
              >
                {axisNumber(value)}
              </text>
            ))}
          </stack>
        )}
        <stack className={pie ? 'grow basis-0 min-w-0 h-52' : 'grow basis-0 min-w-0 h-36'}>
          {!pie &&
            scale.ticks.map((value, index) => (
              <vector
                key={`grid-${index}`}
                viewBox={[0, 0, 1, 1]}
                className={
                  value === 0
                    ? 'absolute left-1 right-1 h-px object-fill text-faint'
                    : 'absolute left-1 right-1 h-px object-fill text-faint'
                }
                style={{ top: tickY(value) }}
                paths={[
                  {
                    d: 'M0 .5H1',
                    fill: 'none',
                    stroke: 'currentColor',
                    strokeWidth: 1,
                    strokeDasharray: value === 0 ? [] : [4, 2],
                    vectorEffect: 'non-scaling-stroke',
                  },
                ]}
              />
            ))}
          <plot
            data-testid={`chart-plot-${data.kind}`}
            kind={data.kind}
            values={values.length ? values : [0]}
            colors={colors.length ? colors : ['chart-1/0']}
            domainMin={pie ? undefined : scale.min}
            domainMax={pie ? undefined : scale.max}
            pieLayout={pie ? { center: [0.5, 0.44], innerRadius: 0.4, outerRadius: 0.68, emphasisScale: 5 } : undefined}
            smooth={data.kind === 'line'}
            area={data.kind === 'line'}
            markerEmphasisScale={data.kind === 'line' ? 1.1 : undefined}
            selectedIndex={visible.indexOf(selected.value)}
            onSelect={index => selected.set(visible[index] ?? -1)}
            aria-label={`${data.title}, ${data.kind} chart. ${description}`}
            className={pie ? 'absolute inset-0' : 'absolute left-0 right-0 top-2'}
            style={pie ? undefined : { height: plotHeight }}
          />
          {!pie && item && (
            <grid className="absolute left-0 right-0 px-1" style={{ top: axisTop, height: axisSpan, gridTemplateColumns: data.series.length }}>
              {data.series.map((_, index) => (
                <column key={`pointer-${index}`} className={selected.value === index ? 'h-full bg-line-strong/20' : 'h-full'} />
              ))}
            </grid>
          )}
          {pie && (
            <row data-testid="chart-legend" className="absolute left-0 right-0 bottom-1 flex-wrap justify-center px-2 gap-3">
              {data.series.map((entry, index) => (
                <button
                  key={`${index}-${entry.label}`}
                  data-testid={`chart-series-${index}`}
                  aria-label={`${entry.label}: ${entry.value}, ${hidden.value.includes(index) ? 'hidden, show series' : 'visible, hide series'}`}
                  onClick={() => toggle(index)}
                  className="min-w-0 p-0 rounded bg-transparent text-muted"
                >
                  <row className="items-center gap-1.5">
                    <spacer
                      className="w-2 h-2 rounded-full"
                      style={{ backgroundColor: hidden.value.includes(index) ? 'text-faint' : palette[index % palette.length] }}
                    />
                    <text className={hidden.value.includes(index) ? 'text-[11px] leading-none text-faint' : 'text-[11px] leading-none'}>
                      {entry.label}
                    </text>
                  </row>
                </button>
              ))}
            </row>
          )}
          {item && (
            <text data-testid="chart-tooltip" className="absolute top-1 right-1 px-2 py-1 rounded-lg border border-line bg-surface text-xs shadow-sm">
              {`${item.label}: ${pie ? `${percentages[visible.indexOf(selected.value)] ?? 0}%` : item.value}`}
            </text>
          )}
        </stack>
      </row>
      {!pie && (
        <row className="px-2 pb-2 gap-1">
          <spacer className="w-7" />
          <grid data-testid="chart-x-axis" className="grow basis-0 min-w-0 px-1" style={{ gridTemplateColumns: data.series.length }}>
            {data.series.map((entry, index) => (
              <text
                key={`${index}-${entry.label}`}
                data-testid={`chart-label-${index}`}
                className="text-[11px] text-center text-muted line-clamp-1"
              >
                {entry.label}
              </text>
            ))}
          </grid>
        </row>
      )}
    </column>
  );
}
