import { useAction, useLocal } from '@lingxia/view';
import '../../theme/index.css';
import { Button, Page, Section } from '../../shared/ui';
import type { View } from './$types';

export function render({ data, actions }: View) {
  const draft = useLocal('clipboard-draft', 'Hello from lx.clipboard');
  const writeText = useAction(actions.writeText);
  const readText = useAction(actions.readText);
  const writeTypedText = useAction(actions.writeTypedText);
  const writeSampleImage = useAction(actions.writeSampleImage);
  const chooseAndWriteImage = useAction(actions.chooseAndWriteImage);
  const peekTypes = useAction(actions.peekTypes);
  const readAll = useAction(actions.readAll);
  const clearClipboard = useAction(actions.clearClipboard);
  const calls = [writeText, readText, writeTypedText, writeSampleImage, chooseAndWriteImage, peekTypes, readAll, clearClipboard];
  const busy = calls.some(call => call.pending);
  const failure = calls.some(call => call.error) ? 'The action failed. Please try again.' : '';
  const status = failure || (busy ? 'Working...' : String(data.statusText || 'Ready'));
  return (
    <Page intro="lx.clipboard reads and writes the system clipboard. Write never toasts.">
      <Section title="Status">
        <text data-testid="clipboard-status" className={failure ? 'text-sm text-danger' : 'text-sm'}>
          {status}
        </text>
        <text data-testid="clipboard-types-value" className="text-xs text-muted">{`types: ${String(data.typesText || 'Not peeked')}`}</text>
      </Section>
      <Section title="Text" description="writeText / readText. An empty string is a valid write, not clear().">
        <textarea
          data-testid="clipboard-draft"
          aria-label="Text to copy"
          bind={draft}
          placeholder="Text to copy"
          className="h-20 px-3 py-2 rounded-lg border border-line-strong bg-surface text-sm"
        />
        <view className="grid gap-3 grid-cols-2">
          <Button testId="clipboard-write-text" disabled={busy} onClick={() => writeText.run({ text: draft.value })}>
            Write text
          </Button>
          <Button variant="secondary" testId="clipboard-read-text" disabled={busy} onClick={() => readText.run()}>
            Read text
          </Button>
        </view>
        <Button
          variant="secondary"
          testId="clipboard-write-typed"
          disabled={busy}
          onClick={() => writeTypedText.run({ text: draft.value })}
        >
          Write typed item
        </Button>
        {!!data.readTextValue && (
          <text data-testid="clipboard-read-value" className="px-3 py-2 rounded-lg bg-surface-sunken text-xs text-muted break-all">
            {String(data.readTextValue)}
          </text>
        )}
      </Section>
      <Section title="Image" description="Write a 1×1 sample PNG, or pick one with lx.pick.media.">
        <view className="grid gap-3 grid-cols-2">
          <Button testId="clipboard-write-sample-image" disabled={busy} onClick={() => writeSampleImage.run()}>
            Write sample PNG
          </Button>
          <Button variant="secondary" testId="clipboard-choose-image" disabled={busy} onClick={() => chooseAndWriteImage.run()}>
            Choose image
          </Button>
        </view>
        {!!data.imagePath && (
          <view className="items-center gap-2">
            <image
              data-testid="clipboard-image"
              src={String(data.imagePath)}
              alt="Image from clipboard"
              className="w-40 h-40 rounded border border-line object-contain"
            />
            <text className="text-xs text-muted break-all">{String(data.imagePath)}</text>
          </view>
        )}
      </Section>
      <Section title="Inspect" description="Peek types without the payload, read every representation, or clear.">
        <view className="grid gap-3 grid-cols-3">
          <Button variant="secondary" testId="clipboard-types" disabled={busy} onClick={() => peekTypes.run()}>
            Types
          </Button>
          <Button testId="clipboard-read-all" disabled={busy} onClick={() => readAll.run()}>
            Read
          </Button>
          <Button variant="destructive" testId="clipboard-clear" disabled={busy} onClick={() => clearClipboard.run()}>
            Clear
          </Button>
        </view>
      </Section>
    </Page>
  );
}
