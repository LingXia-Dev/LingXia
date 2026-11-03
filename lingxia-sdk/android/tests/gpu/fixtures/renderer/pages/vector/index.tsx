import { useLocal, useView } from '@lingxia/view';

const icons = [
  ['M6 19a4 4 0 0 1-.4-7.98A5.5 5.5 0 0 1 16 8.5h.5a4.5 4.5 0 1 1 .5 9H6z'],
  ['M21 10c0 7-9 13-9 13s-9-6-9-13a9 9 0 0 1 18 0z', 'M15 10A3 3 0 1 0 9 10A3 3 0 1 0 15 10Z'],
  ['M7 2H17A2 2 0 0 1 19 4V20A2 2 0 0 1 17 22H7A2 2 0 0 1 5 20V4A2 2 0 0 1 7 2Z', 'M12 18h.01'],
];

export function render() {
  const { env } = useView();
  const alternate = useLocal('vector-alternate', false);
  const tint = alternate.value ? 'text-blue-600' : 'text-red-600';
  return <column className="bg-white p-4 gap-3 text-black" style={{ paddingTop: env.insets.top + 16 }}>
    <button testId="vector-toggle" onClick={() => alternate.set(!alternate.value)} className="w-60 h-12 bg-blue-600 text-white">{alternate.value ? 'Blue' : 'Red'}</button>
    <row className="gap-3">
      <button testId="vector-fill" accessibilityLabel="Filled circle" className={`w-20 h-20 p-0 bg-white rounded-none ${tint}`}>
        <vector className="w-full h-full" paths={[{ d: 'M18 12A6 6 0 1 0 6 12A6 6 0 1 0 18 12Z' }]} />
      </button>
      <button testId="vector-stroke" accessibilityLabel="Outlined square" className={`w-20 h-20 p-0 bg-white rounded-none ${tint}`}>
        <vector className="w-full h-full" paths={[{ d: 'M3 3h18v18H3z', fill: 'none', stroke: 'currentColor', strokeWidth: 1.5 }]} />
      </button>
      <button testId="vector-multicolor" accessibilityLabel="Layered paths" className="w-20 h-20 p-0 bg-white rounded-none">
        <vector className="w-full h-full" paths={[{ d: 'M0 0h24v24H0z', fill: '#ff0000' }, { d: 'M12 0h12v24H12z', fill: '#0000ff' }]} />
      </button>
    </row>
    <button testId="vector-fit" accessibilityLabel="Centered viewBox" className="w-40 h-20 p-0 bg-white rounded-none">
      <vector className="w-full h-full" viewBox={[10, 20, 24, 24]} paths={[{ d: 'M10 20h24v24H10z', fill: '#00ff00' }]} />
    </button>
    <scroll testId="vector-scroll" className="w-20 h-20 bg-white">
      <vector className="w-20 h-40 shrink-0" viewBox={[0, 0, 24, 48]} paths={[{ d: 'M0 0h24v24H0z', fill: '#ff0000' }, { d: 'M0 24h24v24H0z', fill: '#0000ff' }]} />
    </scroll>
    <row className="gap-3">{icons.map((paths, index) => <vector key={String(index)} className="w-12 h-12 text-gray-900"
      paths={paths.map(d => ({ d, fill: 'none', stroke: 'currentColor', strokeWidth: 1.5 }))} />)}</row>
  </column>;
}
