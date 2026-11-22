import '../../theme/index.css';
import type { View } from './$types';

export function render({ data }: View) {
  return (
    <page data-testid="spec-stall" className="bg-canvas items-center justify-center">
      <text data-testid="spec-stall-ready" className="text-sm text-muted">{data.ready ? 'First state arrived' : ''}</text>
    </page>
  );
}
