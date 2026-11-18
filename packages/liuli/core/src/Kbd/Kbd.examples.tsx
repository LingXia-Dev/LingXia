import { Kbd } from './Kbd';

const shortcuts = [
  { key: 'search', label: 'Search notes', keys: ['Ctrl', 'K'], spoken: 'Control K' },
  { key: 'new', label: 'New note', keys: ['Ctrl', 'N'], spoken: 'Control N' },
  { key: 'archive', label: 'Move to archive', keys: ['Ctrl', 'Shift', 'A'], spoken: 'Control Shift A' },
  { key: 'close', label: 'Close the panel', keys: ['Esc'], spoken: 'Escape' },
] as const;

/** A list of shortcuts: what each does, and its keys at the end of the row. */
export function Shortcuts() {
  return (
    <view className="gap-3">
      {shortcuts.map(shortcut => (
        <view key={shortcut.key} className="flex-row items-center justify-between gap-4">
          <text className="text-body text-text">{shortcut.label}</text>
          <Kbd keys={shortcut.keys} label={shortcut.spoken} data-testid={shortcut.key} />
        </view>
      ))}
    </view>
  );
}

/** In a line of help, at the size of the text it sits with. */
export function Hint() {
  return (
    <view className="gap-2">
      <view className="flex-row items-center gap-2">
        <Kbd keys={['Enter']} size="md" data-testid="enter" />
        <text className="text-body text-muted">to send</text>
      </view>
      <view className="flex-row items-center gap-2">
        <Kbd keys={['Shift', 'Enter']} size="md" />
        <text className="text-body text-muted">for a new line</text>
      </view>
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return <Kbd keys={['Ctrl', 'K']} size="md" label="Control K" />;
}
