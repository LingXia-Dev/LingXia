import { useLocal, useView } from '@lingxia/ui';
export function render() {
  const { env } = useView();
  const bordered = useLocal('padded-bordered', false);
  const width = useLocal('padded-width', 240);
  const text = 'A standalone window with no sidebar or shell. Desktop only.';
  const box = bordered.value ? 'px-3 py-2 border-2 border-blue-500 bg-gray-100' : 'px-3 py-2 bg-gray-100';
  return <column className="bg-white text-gray-900 gap-4 p-4" style={{ paddingTop: env.insets.top + 16 }}>
    <button testId="padded-toggle" onClick={() => { if (bordered.value) width.set(width.value + 4); bordered.set(!bordered.value); }} className="h-12 bg-blue-500 text-white">Toggle border</button>
    <text className="text-sm">Direct padded text</text>
    <text testId="padded-direct" style={{ width: width.value }} className={`${box} text-xs leading-5`}>{text}</text>
    <text className="text-sm">Equivalent container</text>
    <column testId="padded-container" style={{ width: width.value }} className={box}><text testId="padded-inner" className="text-xs leading-5">{text}</text></column>
  </column>;
}
