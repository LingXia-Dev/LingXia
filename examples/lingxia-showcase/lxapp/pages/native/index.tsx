import { t, useAction, useLocal, useStream } from '@lingxia/view';
import '../../theme/index.css';
import { said } from '../../shared/lib/said';
import { Button, Page, Section } from '../../shared/ui';
import type { View } from './$types';

export function render({ data, actions }: View) {
  const title = useLocal('native-title', '');
  const progress = useLocal('native-progress', '');
  const add = useAction(actions.add);
  const findMissing = useAction(actions.findMissing);
  const exporting = useStream('export', actions.export, {
    onData: step => progress.set(t('native.exportStep', { done: String(step.done), total: String(step.total) })),
  });
  const streaming = exporting.status === 'streaming';
  const busy = add.pending || findMissing.pending || streaming;
  const submit = () => {
    if (busy) return;
    void add.run({ title: title.value }).then(outcome => outcome.ok && title.set(''));
  };
  return (
    <Page intro={t('native.intro')}>
      <Section title={t('native.status')}>
        <text data-testid="native-status" className="text-sm">
          {said(String(data.status), data.statusArgs as Record<string, unknown>)}
        </text>
      </Section>
      <Section title={t('native.call')} description={t('native.callDetail')}>
        <input
          data-testid="native-title"
          aria-label={t('native.titleLabel')}
          placeholder={t('native.titleLabel')}
          bind={title}
          onSubmit={submit}
          className="min-h-11 px-3 rounded-lg border border-line-strong bg-surface text-sm"
        />
        <view className="grid gap-3 grid-cols-2">
          <Button testId="native-add" disabled={busy} onClick={submit}>{t('native.add')}</Button>
          <Button variant="secondary" testId="native-missing" disabled={busy} onClick={() => findMissing.run()}>
            {t('native.findMissing')}
          </Button>
        </view>
        {data.notes.map(note => (
          <text key={String(note.id)} data-testid={`native-note-${note.id}`} className="text-sm">{`${note.id}. ${note.title}`}</text>
        ))}
      </Section>
      <Section title={t('native.stream')} description={t('native.streamDetail')}>
        <Button
          testId="native-export"
          disabled={busy || data.notes.length === 0}
          onClick={() => { progress.set(''); exporting.start(); }}
        >
          {t('native.export')}
        </Button>
        {progress.value ? <text data-testid="native-progress" className="text-xs text-muted">{progress.value}</text> : null}
        {data.exported ? (
          <text data-testid="native-exported" className="px-3 py-2 rounded-lg bg-surface-sunken text-xs">{data.exported}</text>
        ) : null}
      </Section>
    </Page>
  );
}
