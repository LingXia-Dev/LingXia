import { useLocal } from '@lingxia/view';
import { Held, unused } from '../example';
import { Item } from '../Item/Item';
import { Switch } from './Switch';

/** A setting that applies the moment it flips. */
export function Basic() {
  const sync = useLocal('sync', true);
  return <Switch bind={sync} label="Sync over mobile data" description="Photos wait for Wi-Fi either way." data-testid="sync" />;
}

/** Among list rows: the row's padding and height. The stage's own padding is undone so the rows meet its edges. */
export function InList() {
  const alerts = useLocal('alerts', true);
  const digest = useLocal('digest', false);
  const sounds = ['Chime', 'Bell', 'None'];
  const sound = useLocal('sound', 0);
  return (
    <view className="-m-5 divide-y divide-text/8">
      {/* An app opens a list of sounds here; the row steps through them so the press shows. */}
      <Item label="Sound" endContent={<text className="text-body">{sounds[sound.value]}</text>} onClick={() => sound.set((sound.value + 1) % sounds.length)} hasChevron />
      <Switch bind={alerts} label="Mentions" variant="row" data-testid="mentions" />
      <Switch bind={digest} label="Weekly digest" variant="row" data-testid="digest" />
    </view>
  );
}

export function Sizes() {
  return (
    <view className="gap-3">
      <Held value={true}>{(on, set) => <Switch value={on} onChange={set} label="Small, for dense rows" size="sm" />}</Held>
      <Held value={true}>{(on, set) => <Switch value={on} onChange={set} label="Medium" />}</Held>
    </view>
  );
}

export function StateOff() {
  return <Held value={false}>{(on, set) => <Switch value={on} onChange={set} label="Wi-Fi" />}</Held>;
}

export function StateOn() {
  return <Held value={true}>{(on, set) => <Switch value={on} onChange={set} label="Wi-Fi" />}</Held>;
}

export function StateDisabledOff() {
  return <Switch value={false} onChange={unused} label="Wi-Fi" isDisabled />;
}

export function StateDisabledOn() {
  return <Switch value onChange={unused} label="Wi-Fi" isDisabled />;
}

/** The home tile. */
export function Specimen() {
  return <Switch value onChange={unused} label="Wi-Fi" className="w-32" />;
}

/** While the app applies a change that takes a moment, the switch waits and says so. */
export function Applying() {
  return <Switch value onChange={unused} label="Back up over Wi-Fi" description="Turning on: checking your storage." isLoading data-testid="applying" />;
}
