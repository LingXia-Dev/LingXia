import { useLocal, useView, type View } from '@lingxia/view';
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
    <view className="-m-5 divide-y divide-line">
      {/* An app opens a list of sounds here; the row steps through them so the press shows. */}
      <Item label="Sound" endContent={<text className="text-body">{sounds[sound.value]}</text>} onClick={() => sound.set((sound.value + 1) % sounds.length)} hasChevron />
      <Switch bind={alerts} label="Mentions" variant="row" data-testid="mentions" />
      <Switch bind={digest} label="Weekly digest" variant="row" data-testid="digest" />
    </view>
  );
}

/** In a row that names the setting already: the label is the switch's name, not drawn. */
export function LabelHidden() {
  const pinned = useLocal('pinned', false);
  return (
    <view className="flex-row items-center gap-3">
      <text className="grow text-body text-text">Keep this chat at the top</text>
      <Switch bind={pinned} label="Pin chat" isLabelHidden data-testid="pin" />
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

type Actions = { publish(): Promise<void> };

/** `changeAction` saves the change elsewhere: the switch flips at once and is busy meanwhile; a failure flips it back and says so. */
export function Saved() {
  const { actions } = useView() as View<Actions>;
  const share = useLocal('share', false);
  const fail = useLocal('fail', true);
  return (
    <view className="gap-4">
      <Switch label="Fail the request" bind={fail} data-testid="saved-fail" />
      <Switch
        label="Share usage data"
        description="Saved to your account, on every device."
        bind={share}
        changeAction={async () => {
          await actions.publish();
          if (fail.value) throw Error('offline');
        }}
        data-testid="saved"
      />
    </view>
  );
}

/**
 * Label hidden, the failure has no line of its own: it is announced, describes the switch and is marked before
 * the track; in an Item it is drawn under the row's words instead. Both fail here, so flipping shows it.
 */
export function SavedLabelHidden() {
  const pinned = useLocal('pinned', false);
  const backup = useLocal('backup', false);
  const offline = async () => { throw Error('offline'); };
  return (
    // The stage's padding is undone so both rows take an Item's insets and line up.
    <view className="-m-5 divide-y divide-line">
      <view className="flex-row items-center gap-3 px-4 py-2.5 min-h-control-lg">
        <text className="grow text-body text-text">Keep this chat at the top</text>
        <Switch bind={pinned} label="Pin chat" isLabelHidden changeAction={offline} data-testid="pin-saved" />
      </view>
      <Item label="Back up photos" description="Over Wi-Fi only." endContent={<Switch bind={backup} label="Back up photos" isLabelHidden changeAction={offline} data-testid="backup-saved" />} data-testid="backup-row" />
    </view>
  );
}
