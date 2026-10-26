import { useLocal, type View } from '@lingxia/ui';
import '../../theme/index.css';
import { Button, Page, Section } from '../../shared/ui';

export function render({ data, actions }: View) {
  const draft = useLocal('clipboard-draft', 'Hello from lx.clipboard');
  const busy = useLocal('clipboard-busy', false);
  const failure = useLocal('clipboard-error', '');
  const run = async (action: () => Promise<unknown>) => {
    if (busy.value) return;
    busy.set(true);
    failure.set('');
    try {
      await action();
    } catch {
      failure.set('The action failed. Please try again.');
    } finally {
      busy.set(false);
    }
  };
  const status = failure.value || (busy.value ? 'Working...' : String(data.statusText || 'Ready'));
  return (
    <Page title="Clipboard" subtitle="lx.clipboard reads and writes the system clipboard. Write never toasts.">
      <Section title="Status">
        <text testId="clipboard-status" className={failure.value ? 'text-sm text-danger' : 'text-sm'}>
          {status}
        </text>
        <text testId="clipboard-types-value" className="text-xs text-muted">{`types: ${String(data.typesText || 'Not peeked')}`}</text>
      </Section>
      <Section title="Text" description="writeText / readText. An empty string is a valid write, not clear().">
        <textarea
          testId="clipboard-draft"
          accessibilityLabel="Text to copy"
          bind={draft}
          placeholder="Text to copy"
          className="h-20 px-3 py-2 rounded-lg border border-line-strong bg-surface text-sm"
        />
        <grid className="gap-3 grid-cols-2">
          <Button testId="clipboard-write-text" disabled={busy.value} onClick={() => run(() => actions.writeText({ text: draft.value }))}>
            Write text
          </Button>
          <Button variant="secondary" testId="clipboard-read-text" disabled={busy.value} onClick={() => run(() => actions.readText())}>
            Read text
          </Button>
        </grid>
        <Button
          variant="secondary"
          testId="clipboard-write-typed"
          disabled={busy.value}
          onClick={() => run(() => actions.writeTypedText({ text: draft.value }))}
        >
          Write typed item
        </Button>
        {!!data.readTextValue && (
          <text testId="clipboard-read-value" className="px-3 py-2 rounded-lg bg-surface-sunken text-xs text-muted break-all">
            {String(data.readTextValue)}
          </text>
        )}
      </Section>
      <Section title="Image" description="Write a 1×1 sample PNG, or pick one with lx.chooseMedia.">
        <grid className="gap-3 grid-cols-2">
          <Button testId="clipboard-write-sample-image" disabled={busy.value} onClick={() => run(() => actions.writeSampleImage())}>
            Write sample PNG
          </Button>
          <Button variant="secondary" testId="clipboard-choose-image" disabled={busy.value} onClick={() => run(() => actions.chooseAndWriteImage())}>
            Choose image
          </Button>
        </grid>
        {!!data.imagePath && (
          <column className="items-center gap-2">
            <image
              testId="clipboard-image"
              src={String(data.imagePath)}
              alt="Image from clipboard"
              className="w-40 h-40 rounded border border-line object-contain"
            />
            <text className="text-xs text-muted break-all">{String(data.imagePath)}</text>
          </column>
        )}
      </Section>
      <Section title="Inspect" description="Peek types without the payload, read every representation, or clear.">
        <grid className="gap-3 grid-cols-3">
          <Button variant="secondary" testId="clipboard-types" disabled={busy.value} onClick={() => run(() => actions.peekTypes())}>
            Types
          </Button>
          <Button testId="clipboard-read-all" disabled={busy.value} onClick={() => run(() => actions.readAll())}>
            Read
          </Button>
          <Button variant="destructive" testId="clipboard-clear" disabled={busy.value} onClick={() => run(() => actions.clearClipboard())}>
            Clear
          </Button>
        </grid>
      </Section>
    </Page>
  );
}
