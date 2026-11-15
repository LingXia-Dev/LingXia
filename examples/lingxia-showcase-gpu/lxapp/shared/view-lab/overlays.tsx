// Overlays demos of the View catalogue: each is the body of one demo.
import { useLocal } from '@lingxia/view';
import { Button } from '../ui';
import { Readout, useSay } from './demo';
import { labels, readouts } from './text';

// One note that opens at the inline end of whichever button asked for it.
export function BesideDemo() {
  const which = useLocal<'none' | 'start' | 'end' | 'wide'>('lab-beside-which', 'none');
  const t = useSay();
  const toggle = (name: 'start' | 'end' | 'wide') => () => which.set(which.value === name ? 'none' : name);
  return (
    <>
      <view className="flex-row justify-between">
        <view id="lab-beside-start"><Button testId="lab-beside-start" size="sm" variant="secondary" onClick={toggle('start')}>{t(labels.sideStart)}</Button></view>
        <view id="lab-beside-end"><Button testId="lab-beside-end" size="sm" variant="secondary" onClick={toggle('end')}>{t(labels.sideEnd)}</Button></view>
      </view>
      <view id="lab-beside-wide"><Button testId="lab-beside-wide" size="sm" variant="secondary" onClick={toggle('wide')}>{t(labels.sideWide)}</Button></view>
      <view id="lab-beside-note" popover="manual" anchor={`lab-beside-${which.value === 'none' ? 'start' : which.value}`} open={which.value !== 'none'}
        onToggle={next => { if (!next) which.set('none'); }} style={{ positionArea: 'inline-end span-block-end' }}
        role="tooltip" data-testid="lab-beside-note" className="w-32 rounded-lg border border-line-strong bg-surface p-2">
        <text className="text-xs">{t(labels.sideNote)}</text>
      </view>
      <Readout testId="lab-beside-state" state={`open=${which.value}`}>{t(readouts.beside(which.value))}</Readout>
    </>
  );
}

/** The Overlays group: every demo the text table names, drawn by its body. */
export const overlayDemos = {
  beside: BesideDemo,
};
