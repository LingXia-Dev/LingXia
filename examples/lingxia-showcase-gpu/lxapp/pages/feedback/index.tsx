import { useLocal, type View } from '@lingxia/ui';
import '../../theme/index.css';
import { Button, ErrorState, Field, Header, Page, Segmented } from '../../shared/ui';

const categories = ['Product', 'Bug', 'Idea'].map(label => ({ value: label.toLowerCase(), label }));

export function render({ actions }: View) {
  const category = useLocal('category', 'product');
  const message = useLocal('message', '');
  const email = useLocal('email', '');
  const submitting = useLocal('submitting', false);
  const failure = useLocal('error', '');
  const canSubmit = Boolean(message.value.trim()) && !submitting.value;
  const submit = async () => {
    if (!canSubmit) return;
    submitting.set(true);
    failure.set('');
    try {
      await actions.submitFeedback({
        category: categories.find(item => item.value === category.value)?.label,
        message: message.value,
        email: email.value,
      });
    } catch (cause) {
      failure.set(cause instanceof Error ? cause.message : String(cause));
    } finally {
      submitting.set(false);
    }
  };
  return (
    <Page>
      <Header title="Help us improve" subtitle="Share a bug, an idea, or anything that felt confusing in the showcase." />
      <column className="gap-2">
        <text className="text-sm font-medium">What is this about?</text>
        <Segmented
          testId="feedback"
          value={category.value}
          options={categories}
          disabled={submitting.value}
          onChange={value => category.set(value)}
        />
      </column>
      <column className="gap-1.5">
        <text className="text-sm font-medium">Your feedback</text>
        <textarea
          data-testid="feedback-message"
          aria-label="Your feedback"
          bind={message}
          disabled={submitting.value}
          placeholder="What happened, and what would you prefer?"
          className="h-44 px-4 py-3 rounded-2xl border border-line-strong bg-surface text-sm leading-6"
        />
      </column>
      <Field
        testId="feedback-email"
        label="Email (optional)"
        bind={email}
        disabled={submitting.value}
        type="email"
        placeholder="you@example.com"
      />
      <Button testId="feedback-submit" disabled={!canSubmit} onClick={submit}>
        {submitting.value ? 'Sending…' : 'Send feedback'}
      </Button>
      <ErrorState testId="feedback-error" error={failure.value || null} />
      <text className="text-center text-xs text-muted">Demo only — submissions are written to the Logic log.</text>
    </Page>
  );
}
