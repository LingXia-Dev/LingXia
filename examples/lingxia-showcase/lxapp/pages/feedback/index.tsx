import { t, useLocal, type MessageKey } from '@lingxia/view';
import '../../theme/index.css';
import { Button, ErrorState, Field, Header, Page, Segmented } from '../../shared/ui';
import type { View } from './$types';


export function render({ actions }: View) {
  const categories = ['product', 'bug', 'idea'].map(value => ({ value, label: t(`feedback.category.${value}` as MessageKey) }));
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
      <Header title={t('feedback.title')} subtitle={t('feedback.subtitle')} />
      <view className="gap-2">
        <text className="text-sm font-medium">{t('feedback.about')}</text>
        <Segmented
          testId="feedback"
          value={category.value}
          options={categories}
          disabled={submitting.value}
          onChange={value => category.set(value)}
        />
      </view>
      <view className="gap-1.5">
        <text className="text-sm font-medium">{t('feedback.message')}</text>
        <textarea
          data-testid="feedback-message"
          aria-label={t('feedback.message')}
          bind={message}
          disabled={submitting.value}
          placeholder={t('feedback.messagePlaceholder')}
          className="h-44 px-4 py-3 rounded-2xl border border-line-strong bg-surface text-sm leading-6"
        />
      </view>
      <Field
        testId="feedback-email"
        label={t('feedback.email')}
        bind={email}
        disabled={submitting.value}
        type="email"
        placeholder="you@example.com"
      />
      <Button testId="feedback-submit" disabled={!canSubmit} onClick={submit}>
        {submitting.value ? t('feedback.sending') : t('feedback.send')}
      </Button>
      <ErrorState testId="feedback-error" error={failure.value || null} />
      <text className="text-center text-xs text-muted">{t('feedback.demoOnly')}</text>
    </Page>
  );
}
