import { useLocal, useView } from '@lingxia/view';
import { Button } from '../Button/Button';
import { Split } from './Split';

/** Primary + companion split: side-by-side on regular, upper/lower leaves on half-opened tabletop folds, stacked on compact. */
export function LiveStreamSplit() {
  return (
    <view className="h-[320px] overflow-hidden rounded-container border border-text/12">
      <Split
        asideWidth={240}
        asideLabel="Live chat"
        aside={
          <view className="p-3 gap-2">
            <text className="text-footnote font-semibold text-text">Live chat</text>
            <text className="text-caption text-muted">Messages stay beside video on wide screens and below in compact or tabletop.</text>
          </view>
        }
        data-testid="live-split"
      >
        <view className="grow items-center justify-center bg-surface-sunken p-4">
          <text className="text-footnote font-semibold text-text">Video Canvas</text>
        </view>
      </Split>
    </view>
  );
}

/** Document + comments sheet: docked aside on regular, draggable BottomSheet on compact. */
export function DocumentSheetSplit() {
  const { env } = useView();
  const commentsOpen = useLocal('comments-open', false);
  const isCompact = env.sizeClass !== 'regular';
  return (
    <view className="h-[320px] overflow-hidden rounded-container border border-text/12">
      <Split
        compact="sheet"
        bind={commentsOpen}
        asideLabel="Comments"
        asideWidth={240}
        aside={
          <view className="p-3 gap-2">
            <text className="text-caption text-muted">Review notes and inline discussion.</text>
          </view>
        }
        data-testid="doc-split"
      >
        <view className="grow p-4 gap-3">
          <text className="text-body font-semibold text-text">Architecture Spec</text>
          {isCompact && (
            <Button
              label="Comments"
              variant="secondary"
              width="content"
              onClick={() => commentsOpen.set(true)}
              data-testid="doc-open-comments"
            />
          )}
        </view>
      </Split>
    </view>
  );
}

/** Tile specimen showing a primary canvas beside its companion aside pane. */
export function Specimen() {
  return (
    <view className="w-full flex-row overflow-hidden rounded-control border border-text/12 bg-canvas">
      <view className="grow shrink basis-0 p-2.5 items-center justify-center bg-surface-sunken">
        <text className="text-caption font-semibold text-text">Canvas</text>
      </view>
      <view className="w-[76px] p-2 gap-1 border-s border-text/8">
        <text className="text-caption font-semibold text-text">Chat</text>
        <text className="text-caption text-muted">Live</text>
      </view>
    </view>
  );
}
