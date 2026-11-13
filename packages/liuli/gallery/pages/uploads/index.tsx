import { useLocal, type Child, type VectorPath } from '@lingxia/view';
import { Badge } from '@liulidesign/core/Badge';
import { Button } from '@liulidesign/core/Button';
import { Card } from '@liulidesign/core/Card';
import { ProgressBar, Spinner } from '@liulidesign/core/ProgressBar';
import { GalleryPage, PageSection } from '../../shared/Layout';

const file: VectorPath[] = [{ d: 'M7 3.5h6.5L18 8v12.5H7zM13.5 3.5V8H18', fill: 'none', stroke: 'currentColor', strokeWidth: 1.8, strokeLinejoin: 'round' }];

function FileIcon() {
  return (
    <view className="size-10 items-center justify-center rounded-control bg-surface-sunken">
      <vector aria-hidden paths={file} viewBox={[0, 0, 24, 24]} className="size-5 text-muted" />
    </view>
  );
}

/** The top of every upload card: the file, its size and where it stands. */
function Heading({ name, detail, status }: { name: string; detail: string; status?: Child }) {
  return (
    <view className="flex-row items-center gap-3">
      <FileIcon />
      <view className="grow shrink min-w-0 gap-0.5">
        <view className="flex-row items-center gap-2">
          <text className="grow shrink min-w-0 line-clamp-1 text-body font-medium text-text">{name}</text>
          {status}
        </view>
        <text className="text-footnote text-muted">{detail}</text>
      </view>
    </view>
  );
}

export function render() {
  // Photos moves on each "Send more" press; nothing on this page runs a timer.
  const sent = useLocal('sent', 116);
  const paused = useLocal('paused', false);
  const retried = useLocal('retried', false);

  const photosDone = sent.value >= 182;
  const active = (photosDone || paused.value ? 0 : 1) + (retried.value ? 1 : 0);

  return (
    <GalleryPage>
      <view className="flex-row items-center justify-between gap-3">
        <text role="heading" headingLevel={1} className="text-title font-semibold tracking-tight text-text">Uploads</text>
        <Badge label={active === 0 ? 'Idle' : `${active} active`} tone={active === 0 ? 'neutral' : 'accent'} data-testid="active" />
      </view>

      <PageSection title="In progress">
        <view role="list" aria-label="In progress" className="gap-3" data-testid="in-progress">
          <view role="listitem">
            <Card padding="sm">
              <view className="gap-3">
                <Heading
                  name="Site photos.zip"
                  detail={photosDone ? '182 MB' : `${sent.value} of 182 MB`}
                  status={photosDone ? <Badge label="Uploaded" tone="success" size="sm" /> : paused.value ? <Badge label="Paused" tone="warning" size="sm" /> : undefined}
                />
                {!photosDone && (
                  <>
                    <ProgressBar label="Site photos.zip" isLabelHidden value={sent.value} max={182} data-testid="photos" />
                    <view className="flex-row justify-end gap-2">
                      <Button label={paused.value ? 'Resume' : 'Pause'} size="sm" variant="ghost" onClick={() => paused.set(!paused.value)} data-testid="pause" />
                      <Button label="Send more" size="sm" isDisabled={paused.value} onClick={() => sent.set(Math.min(182, sent.value + 22))} data-testid="send-more" />
                    </view>
                  </>
                )}
              </view>
            </Card>
          </view>

          <view role="listitem">
            <Card padding="sm">
              <view className="gap-3">
                <Heading name="Interview.mov" detail="1.4 GB" />
                <view className="flex-row items-center gap-2">
                  <Spinner label="Waiting" size="sm" />
                  <text className="text-footnote text-muted">Waiting for Wi-Fi</text>
                </view>
              </view>
            </Card>
          </view>

          <view role="listitem">
            <Card padding="sm">
              <view className="gap-3">
                <Heading
                  name="Budget 2026.xlsx"
                  detail={retried.value ? 'Trying again' : 'The connection dropped'}
                  status={retried.value ? undefined : <Badge label="Failed" tone="danger" size="sm" />}
                />
                {retried.value ? (
                  <ProgressBar label="Budget 2026.xlsx" isLabelHidden data-testid="budget" />
                ) : (
                  <view className="flex-row justify-end">
                    <Button label="Retry" size="sm" onClick={() => retried.set(true)} data-testid="retry" />
                  </view>
                )}
              </view>
            </Card>
          </view>
        </view>
      </PageSection>

      <PageSection title="Done today">
        <view role="list" aria-label="Done today" data-testid="done-today">
          <view role="listitem">
            <Card padding="sm">
              <Heading name="Quarterly report.pdf" detail="4.2 MB · 09:12" status={<Badge label="Uploaded" tone="success" size="sm" />} />
            </Card>
          </view>
        </view>
      </PageSection>
    </GalleryPage>
  );
}
