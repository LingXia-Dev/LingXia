import '../../theme/index.css';
import type { View } from './$types';

export function render({ data }: View) {
  return (
    <page data-testid="nav-lab" className="bg-canvas gap-2 p-4">
      <text data-testid="nav-lab-visible" className="text-sm">{data.visibleLog.join(' ')}</text>
      <text data-testid="nav-lab-shows" className="text-sm">{JSON.stringify(data.shows)}</text>
    </page>
  );
}
