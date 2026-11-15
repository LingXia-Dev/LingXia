// Motion demos of the View catalogue: each is the body of one demo.
import { useLocal } from '@lingxia/view';
import { Readout, useSay } from './demo';
import { labels, readouts } from './text';

// The panel keeps its text and changes only its height; the row after it follows by layout motion.
export function PanelDemo() {
  const open = useLocal('lab-panel-open', false);
  const t = useSay();
  return (
    <>
      <button data-testid="lab-panel-toggle" aria-expanded={open.value} onClick={() => open.set(!open.value)}
        className="h-11 px-3 rounded-lg bg-surface-sunken text-sm font-semibold items-start justify-center active:opacity-70">
        {t(labels.panelHeading)}
      </button>
      <view data-testid="lab-panel-box"
        className={`overflow-hidden rounded-lg bg-accent-soft transition-size duration-300 ease-out ${open.value ? 'h-24' : 'h-0'}`}>
        <text className="p-3 text-sm">{t(labels.panelBody)}</text>
      </view>
      <view data-testid="lab-panel-after" className="h-10 px-3 rounded-lg border border-line justify-center transition-layout duration-300 ease-out">
        <text className="text-sm">{t(labels.panelAfter)}</text>
      </view>
      <Readout testId="lab-panel-state" state={`open=${open.value}`}>{t(readouts.panel(open.value))}</Readout>
    </>
  );
}

/** The Motion group: every demo the text table names, drawn by its body. */
export const motionDemos = {
  panel: PanelDemo,
};
