import { useLocal, type View } from '@lingxia/ui';

export function render({ data }: View<object, { options: string[] }>) {
  const mode = useLocal('mode', 'week');
  const notify = useLocal('notify', true);
  const picked = useLocal<string[]>('picked', ['Kids']);
  return (
    <page className="bg-canvas">
      <column className="p-4 gap-3">
        <text role="heading" className="text-xl font-semibold">Schedule</text>
        <text className="text-sm text-muted">Behind the sheet.</text>
      </column>
      <column className="fixed inset-0 bg-scrim" />
      <column className="fixed inset-x-0 bottom-0 p-4 pb-8 gap-4 rounded-t-3xl bg-surface-raised shadow-xl">
        <spacer className="self-center w-10 h-1 rounded-full bg-line-strong" />
        <row className="p-1 gap-1 rounded-xl bg-surface-sunken">
          {['day', 'week', 'month'].map(option => (
            <button key={option} onClick={() => mode.set(option)} className={mode.value === option ? 'grow basis-0 py-2 rounded-lg bg-surface shadow-sm' : 'grow basis-0 py-2 rounded-lg bg-transparent'}>
              <text className="text-sm capitalize">{option}</text>
            </button>
          ))}
        </row>
        <switch bind={notify} className="text-sm">Notify me</switch>
        <column className="gap-2">
          {data.options.map(option => (
            <checkbox key={option} value={picked.value.includes(option)} onChange={on => picked.set(on ? [...picked.value, option] : picked.value.filter(p => p !== option))} className="text-sm">{option}</checkbox>
          ))}
        </column>
        <button onClick={() => {}} className="py-3 rounded-xl bg-accent text-on-accent active:opacity-80">Save</button>
      </column>
    </page>
  );
}
