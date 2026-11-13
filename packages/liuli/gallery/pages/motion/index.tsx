import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { Dialog } from '@liulidesign/core/Dialog';
import { DropdownMenu, DropdownMenuItem } from '@liulidesign/core/DropdownMenu';
import { ProgressBar } from '@liulidesign/core/ProgressBar';
import { GalleryPage, PageSection, Stage } from '../../shared/Layout';

// Each demo uses the components' own motion classes; the page adds no timing of its own.

function Press() {
  return (
    <view className="flex-row gap-3">
      <Button label="Save" variant="primary" width="content" data-testid="press-primary" />
      <Button label="Preview" width="content" data-testid="press-secondary" />
    </view>
  );
}

/** Remounting the panel replays its entrance. */
function Enter() {
  const round = useLocal('round', 0);
  return (
    <view className="gap-4">
      <view key={String(round.value)} className="gap-0.5 rounded-container border border-text/12 bg-surface-raised p-4 shadow-raised transition duration-200 ease-out starting:opacity-0 motion-safe:starting:scale-96">
        <text className="text-body font-medium text-text">Backup finished</text>
        <text className="text-footnote text-muted">3,214 notes are on this device and in the cloud.</text>
      </view>
      <Button label="Replay" size="sm" width="content" onClick={() => round.set(round.value + 1)} data-testid="replay-enter" />
    </view>
  );
}

function DialogDemo() {
  const open = useLocal('open', false);
  return (
    <view className="flex-row">
      <Button label="Open dialog" width="content" onClick={() => open.set(true)} data-testid="open-dialog" />
      <Dialog
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="Turn on backups?"
        description="Your notes are copied to the cloud every night."
        size="sm"
        data-testid="motion-dialog"
        actions={
          <>
            <Button label="Not now" onClick={() => open.set(false)} />
            <Button label="Turn on" variant="primary" onClick={() => open.set(false)} />
          </>
        }
      />
    </view>
  );
}

function Menu() {
  return (
    <view className="flex-row">
      <DropdownMenu label="Open menu" data-testid="open-menu">
        <DropdownMenuItem label="Rename" onClick={() => {}} />
        <DropdownMenuItem label="Duplicate" onClick={() => {}} />
        <DropdownMenuItem label="Move to folder" onClick={() => {}} />
      </DropdownMenu>
    </view>
  );
}

export function render() {
  return (
    <GalleryPage>
      <text className="text-body text-muted">
        Motion is short and eases out: it confirms a touch or shows where something came from, then gets out of the way.
      </text>
      <PageSection title="Press">
        <Stage caption="The fill changes in 150 ms, ease-out. Press and hold to see it">
          <Press />
        </Stage>
      </PageSection>
      <PageSection title="Enter">
        <Stage caption="Fades in and grows from 96% in 200 ms, ease-out">
          <Enter />
        </Stage>
      </PageSection>
      <PageSection title="Dialog">
        <Stage caption="Enters like a panel and leaves the same way, 200 ms">
          <DialogDemo />
        </Stage>
      </PageSection>
      <PageSection title="Menu">
        <Stage caption="Grows from 95% toward its trigger in 150 ms">
          <Menu />
        </Stage>
      </PageSection>
      <PageSection title="Waiting">
        <Stage caption="An unknown amount sweeps the track every 1.2 s">
          <ProgressBar label="Preparing the export" />
        </Stage>
      </PageSection>
    </GalleryPage>
  );
}
