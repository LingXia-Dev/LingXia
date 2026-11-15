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

// A row that asks for its own menu: a held finger or a secondary click, never a tap.
export function MenuDemo() {
  const asked = useLocal('lab-menu-asked', 'none');
  const open = useLocal('lab-menu-open', false);
  const last = useLocal('lab-menu-last', 'none');
  const t = useSay();
  const pick = (name: string) => () => { last.set(name); open.set(false); };
  return (
    <>
      <view id="lab-menu-row" data-testid="lab-menu-row" onContextMenu={event => { asked.set(event.pointer); open.set(true); }}
        className="flex-row items-center px-3 py-3 rounded-xl bg-surface-sunken">
        <text className="text-sm">{t(labels.menuRow)}</text>
      </view>
      <view id="lab-menu-list" popover="auto" anchor="lab-menu-row" open={open.value} onToggle={next => open.set(next)}
        role="menu" aria-label={t(labels.rowMenu)} data-testid="lab-menu-list" className="w-40 rounded-lg border border-line-strong bg-surface p-1">
        <button role="menuitem" data-testid="lab-menu-copy" onClick={pick('copy')} className="px-3 py-2 rounded-md text-sm text-start active:bg-surface-sunken">{t(labels.menuCopy)}</button>
        <button role="menuitem" data-testid="lab-menu-rename" onClick={pick('rename')} className="px-3 py-2 rounded-md text-sm text-start active:bg-surface-sunken">{t(labels.menuRename)}</button>
        <button role="menuitem" data-testid="lab-menu-delete" onClick={pick('delete')} className="px-3 py-2 rounded-md text-sm text-start text-danger active:bg-surface-sunken">{t(labels.menuDelete)}</button>
      </view>
      <Readout testId="lab-menu-state" state={`open=${open.value} asked=${asked.value} last=${last.value}`}>{t(readouts.menu(asked.value, last.value))}</Readout>
    </>
  );
}

// A menu whose zoom items act and leave it open; "Reset" is an ordinary item.
export function StayDemo() {
  const zoom = useLocal('lab-stay-zoom', 100);
  const open = useLocal('lab-stay-open', false);
  const t = useSay();
  const item = 'px-3 py-2 rounded-md text-sm text-start active:bg-surface-sunken focus-visible:bg-surface-sunken';
  return (
    <>
      <button popoverTarget="lab-stay-menu" data-testid="lab-stay-open"
        className="self-start px-3 py-2 rounded-lg border border-line text-sm active:bg-surface-sunken">{t(labels.stayOpen)}</button>
      <view id="lab-stay-menu" popover="auto" role="menu" aria-label={t(labels.stayOpen)} data-testid="lab-stay-menu" onToggle={next => open.set(next)}
        className="w-40 rounded-lg border border-line-strong bg-surface p-1">
        <button role="menuitem" keepOpen data-testid="lab-stay-in" onClick={() => zoom.set(Math.min(200, zoom.value + 10))} className={item}>{t(labels.stayIn)}</button>
        <button role="menuitem" keepOpen data-testid="lab-stay-out" onClick={() => zoom.set(Math.max(50, zoom.value - 10))} className={item}>{t(labels.stayOut)}</button>
        <button role="menuitem" data-testid="lab-stay-reset" onClick={() => zoom.set(100)} className={item}>{t(labels.stayReset)}</button>
      </view>
      <Readout testId="lab-stay-state" state={`open=${open.value} zoom=${zoom.value}`}>{t(readouts.stay(open.value, zoom.value))}</Readout>
    </>
  );
}

// A popover that hears Escape and Back first, and keeps itself open while its draft is kept.
export function RefuseDemo() {
  const open = useLocal('lab-refuse-open', false);
  const keep = useLocal('lab-refuse-keep', true);
  const asked = useLocal('lab-refuse-asked', 0);
  const t = useSay();
  return (
    <>
      <button popoverTarget="lab-refuse-note" data-testid="lab-refuse-open"
        className="self-start px-3 py-2 rounded-lg border border-line text-sm active:bg-surface-sunken">{t(labels.refuseOpen)}</button>
      <view id="lab-refuse-note" popover="auto" role="group" aria-label={t(labels.refuseTitle)} data-testid="lab-refuse-note" onToggle={next => open.set(next)}
        onCloseRequest={() => { asked.set(asked.value + 1); return !keep.value; }}
        className="w-56 gap-2 rounded-lg border border-line-strong bg-surface p-3">
        <text className="text-sm font-semibold">{t(labels.refuseTitle)}</text>
        <switch data-testid="lab-refuse-keep" bind={keep} className="text-sm">{t(labels.refuseKeep)}</switch>
      </view>
      <Readout testId="lab-refuse-state" state={`open=${open.value} asked=${asked.value}`}>{t(readouts.refuse(open.value, asked.value))}</Readout>
    </>
  );
}

// A menu whose "Move to" item opens a second menu at its side; a choice there closes both.
export function SubmenuDemo() {
  const last = useLocal('lab-submenu-last', 'none');
  const t = useSay();
  const item = 'px-3 py-2 rounded-md text-sm text-start active:bg-surface-sunken focus-visible:bg-surface-sunken';
  const panel = 'w-36 rounded-lg border border-line-strong bg-surface p-1';
  return (
    <>
      <button popoverTarget="lab-submenu-menu" data-testid="lab-submenu-open"
        className="h-11 px-3 rounded-lg bg-surface-sunken text-sm font-semibold items-start justify-center active:opacity-70">
        {t(labels.subOpen)}
      </button>
      <view id="lab-submenu-menu" popover="auto" role="menu" aria-label={t(labels.subOpen)} data-testid="lab-submenu-menu" className={panel}>
        <button role="menuitem" data-testid="lab-submenu-rename" onClick={() => last.set('rename')} className={item}>{t(labels.subRename)}</button>
        <button role="menuitem" popoverTarget="lab-submenu-folders" data-testid="lab-submenu-move" className={item}>{t(labels.subMove)}</button>
      </view>
      <view id="lab-submenu-folders" popover="auto" role="menu" aria-label={t(labels.subMove)} data-testid="lab-submenu-folders"
        style={{ positionArea: 'inline-end span-block-end' }} className={panel}>
        <button role="menuitem" data-testid="lab-submenu-inbox" onClick={() => last.set('inbox')} className={item}>{t(labels.subInbox)}</button>
        <button role="menuitem" data-testid="lab-submenu-archive" onClick={() => last.set('archive')} className={item}>{t(labels.subArchive)}</button>
      </view>
      <Readout testId="lab-submenu-state" state={`last=${last.value}`}>{t(readouts.submenu(last.value))}</Readout>
    </>
  );
}

/** The Overlays group: every demo the text table names, drawn by its body. */
export const overlayDemos = {
  menu: MenuDemo,
  submenu: SubmenuDemo,
  stay: StayDemo,
  refuse: RefuseDemo,
  beside: BesideDemo,
};
