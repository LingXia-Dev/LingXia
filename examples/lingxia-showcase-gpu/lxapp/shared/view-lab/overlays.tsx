// Overlays demos of the View catalogue: each is the body of one demo.
import { useLocal, useView } from '@lingxia/view';
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

// Dialogs and popovers draw in the top layer: above the page and unclipped by their demo's card.
const useLast = () => useLocal('overlay-last', 'none');

export function DialogDemo() {
  const open = useLocal('overlay-confirm', false);
  const last = useLast();
  const t = useSay();
  const close = (value: string) => { last.set(value); open.set(false); };
  return (
    <>
      <view className="flex-row"><Button testId="lab-overlay-confirm" size="sm" onClick={() => open.set(true)}>{t(labels.dialog)}</Button></view>
      <Readout testId="lab-dialog-last" state={`last=${last.value}`}>{t(readouts.last(last.value))}</Readout>
      <dialog open={open.value} onClose={() => open.set(false)} aria-label={t(labels.deleteNote)} data-testid="lab-dialog"
        className="w-72 p-4 gap-3 rounded-2xl bg-surface-raised transition duration-200 starting:opacity-0 starting:scale-95 exit:opacity-0 exit:scale-95">
        <text role="heading" headingLevel={3} className="text-base font-semibold">{t(labels.deleteNote)}</text>
        <text className="text-sm text-muted">{t(labels.deleteHelp)}</text>
        <view className="flex-row gap-2 justify-end">
          <Button testId="lab-dialog-cancel" variant="secondary" onClick={() => close('cancel')}>{t(labels.cancel)}</Button>
          <Button testId="lab-dialog-delete" onClick={() => close('delete')}>{t(labels.delete)}</Button>
        </view>
      </dialog>
    </>
  );
}

export function SheetDemo() {
  const open = useLocal('overlay-sheet', false);
  const last = useLast();
  const t = useSay();
  // The sheet covers the bars; its content clears the home indicator or gesture bar.
  const safeBottom = useView().env.safeArea?.bottom ?? 0;
  return (
    <>
      <view className="flex-row"><Button testId="lab-overlay-sheet" size="sm" variant="secondary" onClick={() => open.set(true)}>{t(labels.sheet)}</Button></view>
      <Readout testId="lab-sheet-last" state={`last=${last.value}`}>{t(readouts.last(last.value))}</Readout>
      <dialog open={open.value} onClose={() => open.set(false)} closedby="any" aria-label={t(labels.filters)} data-testid="lab-sheet"
        style={{ paddingBottom: 16 + safeBottom }}
        className="mt-auto mb-0 w-full p-4 gap-3 rounded-t-3xl bg-surface-raised transition-transform duration-300 ease-out starting:translate-y-full exit:translate-y-full">
        <view className="self-center w-10 h-1 rounded-full bg-line-strong" />
        <text role="heading" headingLevel={3} className="text-base font-semibold">{t(labels.filters)}</text>
        <scroll data-testid="lab-sheet-scroll" className="max-h-48">
          {Array.from({ length: 12 }, (_, i) => <text key={`f${i}`} className="py-2 text-sm">{`${t(labels.filterRow)} ${i + 1}`}</text>)}
        </scroll>
        <Button testId="lab-sheet-apply" onClick={() => { last.set('apply'); open.set(false); }}>{t(labels.apply)}</Button>
      </dialog>
    </>
  );
}

export function MoreDemo() {
  const last = useLast();
  const t = useSay();
  return (
    <>
      <view className="flex-row items-center gap-3">
        <view className="grow"><Readout testId="lab-more-last" state={`last=${last.value}`}>{t(readouts.last(last.value))}</Readout></view>
        <button popoverTarget="lab-more-menu" data-testid="lab-overlay-more" aria-label="More" className="rounded-full px-3 py-1.5 bg-surface-sunken text-text">···</button>
      </view>
      <view id="lab-more-menu" popover role="menu" aria-label="Note actions" data-testid="lab-more-menu"
        style={{ positionArea: 'bottom span-inline-start' }} className="w-44 p-1 rounded-xl bg-surface-raised shadow-lg border border-line">
        {['Rename', 'Share', 'Delete'].map(item => (
          <button key={item} role="menuitem" data-testid={`lab-more-${item.toLowerCase()}`} onClick={() => last.set(item)}
            className="px-3 py-2 rounded-lg text-start text-text active:bg-surface-sunken">{item}</button>
        ))}
      </view>
    </>
  );
}

export function PickersDemo() {
  const day = useLocal('lab-picker-day', '');
  const time = useLocal('lab-picker-time', '');
  const t = useSay();
  const field = 'grow basis-0 px-3 py-2 rounded-lg border border-line-strong bg-surface text-sm';
  return (
    <>
      <view className="flex-row gap-2">
        <input data-testid="lab-picker-date" bind={day} aria-label="Date" type="date" placeholder={t(labels.pickDate)} className={field} />
        <input data-testid="lab-picker-time" bind={time} aria-label="Time" type="time" placeholder={t(labels.pickTime)} className={field} />
      </view>
      <Readout testId="lab-picker-values" state={`date=${day.value} time=${time.value}`}>{t(readouts.pickers(day.value, time.value))}</Readout>
    </>
  );
}

/** The Overlays group: every demo the text table names, drawn by its body. */
export const overlayDemos = {
  dialog: DialogDemo,
  bottomSheet: SheetDemo,
  more: MoreDemo,
  pickers: PickersDemo,
  menu: MenuDemo,
  submenu: SubmenuDemo,
  stay: StayDemo,
  refuse: RefuseDemo,
  beside: BesideDemo,
};
