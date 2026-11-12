import { useLocal } from '@lingxia/view';
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
  return (
    <view className="-m-5 divide-y divide-text/8">
      <Item label="Sound" endContent={<text className="text-body">Chime</text>} onClick={() => {}} hasChevron />
      <Switch bind={alerts} label="Mentions" variant="row" data-testid="mentions" />
      <Switch bind={digest} label="Weekly digest" variant="row" data-testid="digest" />
    </view>
  );
}

export function StateOff() {
  return <Switch value={false} onChange={() => {}} label="Wi-Fi" />;
}

export function StateOn() {
  return <Switch value onChange={() => {}} label="Wi-Fi" />;
}

export function StateDisabledOff() {
  return <Switch value={false} onChange={() => {}} label="Wi-Fi" isDisabled />;
}

export function StateDisabledOn() {
  return <Switch value onChange={() => {}} label="Wi-Fi" isDisabled />;
}

/** The home tile. */
export function Specimen() {
  return <Switch value onChange={() => {}} label="Wi-Fi" className="w-32" />;
}
