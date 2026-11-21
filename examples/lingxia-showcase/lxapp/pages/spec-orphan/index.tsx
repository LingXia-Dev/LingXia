import { useLocal } from '@lingxia/view';
import '../../theme/index.css';
import { Button, Page } from '../../shared/ui';

// For specs only: a date field that leaves the page while its picker is open; the picker goes with it.
export function render() {
  const shown = useLocal('lab-orphan-shown', true);
  const day = useLocal('lab-orphan-day', '2026-10-21');
  return (
    <Page testId="spec-orphan">
      {shown.value
        ? <input data-testid="lab-orphan-date" bind={day} aria-label="Day" type="date" className="px-3 py-2 rounded-lg border border-line-strong bg-surface text-sm" />
        : <text data-testid="lab-orphan-gone" className="text-sm text-muted">The field is gone.</text>}
      <Button testId="lab-orphan-toggle" size="sm" variant="secondary" onClick={() => shown.set(!shown.value)}>
        {shown.value ? 'Remove the field' : 'Bring it back'}
      </Button>
    </Page>
  );
}
