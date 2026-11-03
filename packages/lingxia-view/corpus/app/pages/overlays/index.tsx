import { useLocal } from '@lingxia/view';

export function render() {
  const sheet = useLocal('sheet', true);
  return (
    <page className="bg-canvas">
      <column className="p-4 gap-3">
        <row className="items-center">
          <text role="heading" className="grow text-xl font-semibold">Notes</text>
          <button popoverTarget="more" aria-label="More" className="rounded-full px-3 py-1 bg-surface">⋯</button>
        </row>
        <text className="text-sm text-muted">Behind the sheet and the menu.</text>
      </column>
      <view id="more" popover open role="menu" aria-label="Actions" className="mt-24 me-4 ms-auto mb-auto w-48 p-1 rounded-xl bg-surface-raised shadow-lg">
        {['Rename', 'Share', 'Delete'].map(item => (
          <button key={item} role="menuitem" onClick={() => {}} className="px-3 py-2 rounded-lg active:bg-surface-sunken text-start">{item}</button>
        ))}
      </view>
      <dialog open={sheet.value} onClose={() => sheet.set(false)} closedby="any" aria-label="Filters"
        className="mt-auto mb-0 w-full p-4 pb-8 gap-3 rounded-t-3xl bg-surface-raised backdrop:bg-scrim">
        <spacer className="self-center w-10 h-1 rounded-full bg-line-strong" />
        <text className="text-lg font-semibold">Filters</text>
        <scroll className="max-h-40">
          {['Today', 'This week', 'This month', 'Older'].map(item => <text key={item} className="py-2">{item}</text>)}
        </scroll>
        <button onClick={() => sheet.set(false)} className="px-3 py-3 rounded-xl bg-accent text-on-accent">Apply</button>
      </dialog>
    </page>
  );
}
