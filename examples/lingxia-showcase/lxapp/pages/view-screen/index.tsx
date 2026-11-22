import '../../theme/index.css';
import { Button } from '../../shared/ui';
import { useSay } from '../../shared/view-lab/demo';
import { ScreenFacts } from '../../shared/view-lab/screen';
import { labels } from '../../shared/view-lab/text';
import type { View } from './$types';

// A page without a navigation bar: the host's capsule floats over it. An outline marks where
// `env.capsule` says it is, and the header pads its top to keep clear of it.
export function render({ env }: View) {
  const t = useSay();
  const capsule = env.capsule;
  // `<page>` already pads by the insets; the header adds what the capsule needs beyond them.
  const clear = capsule ? Math.max(0, capsule.y + capsule.height + 8 - env.insets.top) : 0;
  return (
    <page data-testid="view-screen" className="bg-canvas">
      {capsule ? (
        <view data-testid="view-screen-capsule-outline" aria-hidden
          className="absolute rounded-full border-2 border-dashed border-danger"
          style={{ left: capsule.x, top: capsule.y, width: capsule.width, height: capsule.height }} />
      ) : null}
      <scroll className="grow basis-0">
        <view data-testid="view-screen-header" className="px-4 pb-4 gap-3" style={{ paddingTop: clear }}>
          <view className="flex-row">
            <Button testId="view-screen-back" size="sm" variant="secondary" to={{ back: 1 }}>{t(labels.envBack)}</Button>
          </view>
          <text className="text-xs text-muted">{t(labels.envOutline)}</text>
          <ScreenFacts />
        </view>
      </scroll>
    </page>
  );
}
