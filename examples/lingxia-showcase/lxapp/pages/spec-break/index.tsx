import '../../theme/index.css';
import { useView } from '@lingxia/view';
import { Button } from '../../shared/ui';

export function render() {
  const { data, actions } = useView();
  if (data.broken) throw new Error('spec-break: this update fails on purpose');
  return (
    <page data-testid="spec-break" className="bg-canvas items-center justify-center gap-4 p-6">
      <text data-testid="spec-break-shown" className="text-sm text-muted">Shown before the update</text>
      <Button testId="spec-break-go" onClick={() => actions.breakView()}>Break the next update</Button>
    </page>
  );
}
