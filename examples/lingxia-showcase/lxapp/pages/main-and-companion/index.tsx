import { useLocal, useView } from '@lingxia/view';
import { Split } from '@liulidesign/core/Split';
import { Button } from '@liulidesign/core/Button';
import { AdaptiveStatus } from '../../shared/adaptive-status';
import '../../theme/index.css';

/** Split alone: the companion beside the main content on a wide screen, below it at a tabletop fold, in a sheet on a phone. */
export function render() {
  const { env } = useView();
  const notesOpen = useLocal('adaptive-notes-open', false);

  return (
    <page title="Main and companion" data-testid="adaptive-page">
      <AdaptiveStatus />
      <Split
        compact="sheet"
        bind={notesOpen}
        asideLabel="Live notes"
        asideWidth={320}
        aside={
          <view className="p-4 gap-2">
            <text className="text-sm font-semibold text-text">Live notes</text>
            <text className="text-xs text-muted">
              Beside the main content on a wide screen, below it at a tabletop fold, in a sheet on a phone.
            </text>
          </view>
        }
        data-testid="adaptive-split"
      >
        <view className="grow p-4 gap-3">
          <text className="text-base font-semibold text-text">Main content</text>
          {/* Only a phone puts the companion in a sheet, so only a phone needs the way to open it. */}
          {env.sizeClass === 'compact' && (
            <Button
              variant="secondary"
              label="Open live notes"
              width="content"
              onClick={() => notesOpen.set(true)}
              data-testid="adaptive-open-notes"
            />
          )}
        </view>
      </Split>
    </page>
  );
}
