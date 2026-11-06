// Renderer-drawn plots: bars with a selection, a pie and a smoothed area line.
const bars = Array.from({ length: 24 }, (_, i) => 20 + ((i * 37) % 40));
const line = Array.from({ length: 30 }, (_, i) => 10 + ((i * 7) % 9));

export function render() {
  return (
    <page className="bg-canvas p-4 gap-4">
      <plot kind="bar" values={bars} colors={['chart-1']} selectedIndex={5} aria-label="Bars" className="h-40" />
      <row className="gap-4">
        <plot kind="pie" values={[45, 25, 15, 10, 5]} colors={['chart-1', 'chart-2', 'chart-3', 'chart-4', 'chart-5']} aria-label="Pie" className="grow basis-0 h-36" />
        <plot kind="line" values={line} colors={['chart-2']} smooth area aria-label="Line" className="grow basis-0 h-36" />
      </row>
    </page>
  );
}
