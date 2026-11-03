import { List, type View } from '@lingxia/view';

type Device = { id: string; name: string; kind: string; online: boolean; usage: number };

export function render({ data }: View<object, { devices: Device[] }>) {
  return (
    <page className="bg-canvas">
      <row className="px-4 py-3 items-center justify-between bg-surface border-b border-line">
        <text role="heading" className="text-lg font-semibold">Devices</text>
        <text className="px-2 py-0.5 rounded-full bg-accent-soft text-xs text-on-accent-soft tabular-nums">{data.devices.filter(d => d.online).length} online</text>
      </row>
      <List data-testid="devices" items={data.devices} getKey={device => device.id} className="grow"
        renderItem={device => (
          <row className="px-4 py-3 gap-3 items-center bg-surface border-b border-line">
            <column className="w-10 h-10 shrink-0 rounded-xl bg-surface-sunken items-center justify-center">
              <text className="text-xs font-semibold uppercase tracking-wide text-muted">{device.kind.slice(0, 2)}</text>
            </column>
            <column className="grow min-w-0 gap-0.5">
              <text className="text-sm font-medium truncate">{device.name}</text>
              <row className="items-center gap-1.5">
                <spacer className={device.online ? 'w-1.5 h-1.5 rounded-full bg-success' : 'w-1.5 h-1.5 rounded-full bg-line-strong'} />
                <text className="text-xs text-muted tabular-nums">{device.usage.toFixed(1)} GB</text>
              </row>
            </column>
            <column className="w-16 h-1.5 shrink-0 rounded-full bg-surface-sunken">
              <column className="h-full rounded-full bg-accent" style={{ width: `${Math.min(100, device.usage * 5)}%` }} />
            </column>
          </row>
        )} />
    </page>
  );
}
