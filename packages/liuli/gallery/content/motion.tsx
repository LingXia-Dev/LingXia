import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { Dialog } from '@liulidesign/core/Dialog';
import { DropdownMenu, DropdownMenuItem } from '@liulidesign/core/DropdownMenu';
import { Said } from '@liulidesign/core/example';
import { ProgressBar } from '@liulidesign/core/ProgressBar';
import { PageSection, Stage } from '../shared/Layout';

// Each demo uses the components' own motion classes; the page adds no timing of its own.

function Press() {
  return (
    <Said>
      {say => (
        <view className="flex-row gap-3">
          <Button label="Save" variant="primary" width="content" onClick={() => say('Saved')} data-testid="press-primary" />
          <Button label="Preview" width="content" onClick={() => say('Opened the preview')} data-testid="press-secondary" />
        </view>
      )}
    </Said>
  );
}

/** Remounting the panel replays its entrance. */
function Enter() {
  const round = useLocal('round', 0);
  return (
    <view className="gap-4">
      <view key={String(round.value)} data-testid="enter-panel" className="gap-0.5 rounded-container border border-text/12 bg-surface-overlay p-4 shadow-raised transition duration-200 ease-out starting:opacity-0 motion-safe:starting:translate-y-2">
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
    <Said>
      {say => (
        <view className="flex-row">
          <DropdownMenu label="Open menu" menu={<>
            <DropdownMenuItem label="Rename" onClick={() => say('Chose Rename')} />
            <DropdownMenuItem label="Duplicate" onClick={() => say('Chose Duplicate')} />
            <DropdownMenuItem label="Move to folder" onClick={() => say('Chose Move to folder')} />
          </>}>
            <Button label="Open menu" endIcon="chevron-down" data-testid="open-menu" />
          </DropdownMenu>
        </view>
      )}
    </Said>
  );
}

export function Content() {
  return (
    <>
      <text className="text-body text-muted">
        Motion is short and eases out: it confirms a touch or shows where something came from, then gets out of the way.
      </text>
      <text className="text-body text-muted">
        What appears on its own, a dialog or a new panel, rises 8 px as it fades in, in duration-base (200 ms). What opens from something, a menu, a popover, a tooltip or suggestions, slides 4 px out from it as it fades, in duration-fast (150 ms); nothing that holds a scroll or a field scales.
      </text>
      <PageSection title="Press">
        <Stage caption="The fill changes in 150 ms, ease-out. Press and hold to see it">
          <Press />
        </Stage>
      </PageSection>
      <PageSection title="Enter">
        <Stage caption="Fades in and rises 8 px in 200 ms (duration-base), ease-out">
          <Enter />
        </Stage>
      </PageSection>
      <PageSection title="Dialog">
        <Stage caption="Rises 8 px as it fades, and leaves the same way, in 200 ms (duration-base)">
          <DialogDemo />
        </Stage>
      </PageSection>
      <PageSection title="Menu">
        <Stage caption="Slides 4 px out from its trigger as it fades, in 150 ms (duration-fast)">
          <Menu />
        </Stage>
      </PageSection>
      <PageSection title="Waiting">
        <Stage caption="An unknown amount sweeps the track every 1.2 s">
          <ProgressBar label="Preparing the export" />
        </Stage>
      </PageSection>
    </>
  );
}
