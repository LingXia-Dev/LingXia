import '../../theme/index.css';
import { useView } from '@lingxia/view';

export function render() {
  const { data } = useView();
  return (
    <page data-testid="spec-stall" className="bg-canvas items-center justify-center">
      <text data-testid="spec-stall-ready" className="text-sm text-muted">{data.ready ? 'First state arrived' : ''}</text>
    </page>
  );
}
