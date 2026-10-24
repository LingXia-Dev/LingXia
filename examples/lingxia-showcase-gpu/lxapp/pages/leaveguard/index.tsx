import { useLeaveGuard, useLocal, type View } from '@lingxia/ui';
import '../../theme/index.css';
import { Button, ErrorState, Field, Page, Section } from '../../shared/ui';

export function render(_view: View) {
  const saved = useLocal('saved', '');
  const draft = useLocal('draft', '');
  const dirty = draft.value !== saved.value;
  const guard = useLeaveGuard('draft', { enabled: dirty });
  return (
    <Page title="Leave guard" subtitle="Edit the draft, then go back or home: the page stays and asks. Save, and it leaves at once.">
      <Section title="Draft">
        <Field testId="leave-draft" label="Draft" bind={draft} placeholder="Type something" />
        <row className="items-center justify-between gap-2">
          <text testId="leave-status" className="text-xs text-muted-foreground">
            {dirty ? 'Unsaved changes' : 'Saved'}
          </text>
          <text testId="leave-requests" className="text-xs text-muted-foreground">{`leave requests: ${guard.requests}`}</text>
        </row>
        <Button testId="leave-save" disabled={!dirty} onClick={() => saved.set(draft.value)}>
          Save
        </Button>
      </Section>
      {guard.pending && (
        <Section testId="leave-confirm" title="Discard unsaved changes?">
          <text testId="leave-question" className="text-sm">
            Leaving now drops the draft.
          </text>
          <grid className="gap-2 grid-cols-2">
            <Button variant="secondary" testId="leave-keep" onClick={() => guard.answer(false)}>
              Keep editing
            </Button>
            <Button variant="destructive" testId="leave-discard" onClick={() => guard.answer(true)}>
              Discard
            </Button>
          </grid>
        </Section>
      )}
      <ErrorState testId="leave-error" error={guard.error} />
    </Page>
  );
}
