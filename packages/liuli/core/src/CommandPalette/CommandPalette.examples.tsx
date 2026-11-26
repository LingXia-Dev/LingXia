import { useLocal } from '@lingxia/view';
import { unused } from '../example';
import { Button } from '../Button/Button';
import { CommandPalette, type CommandGroup, type CommandPaletteProps } from './CommandPalette';

const groups: CommandGroup[] = [
  { label: 'Go to', items: [
    { id: 'all', label: 'All notes', icon: 'file', endText: 'G then A' },
    { id: 'shared', label: 'Shared with me', icon: 'users', endText: 'G then S' },
    { id: 'archive', label: 'Archive', icon: 'folder', keywords: ['old', 'hidden'] },
    { id: 'settings', label: 'Settings', icon: 'settings', description: 'Account, appearance, sync' },
  ] },
  { label: 'Actions', items: [
    { id: 'new', label: 'New note', icon: 'add', shortcut: { keys: ['Ctrl', 'N'], label: 'Control N' } },
    { id: 'share', label: 'Share this note', icon: 'share', keywords: ['send', 'invite'] },
    { id: 'export', label: 'Export as PDF', icon: 'upload', isDisabled: true },
  ] },
];

/** Opened from a button here; an app also opens it from its own shortcut. Type to narrow, arrows to move, Return to run. */
export function Commands() {
  const open = useLocal('open', false);
  const ran = useLocal('ran', '');
  return (
    <view className="items-start gap-2">
      <Button label="Search commands" startIcon="search" onClick={() => open.set(true)} data-testid="palette-open" />
      {ran.value !== '' && <text role="status" className="text-footnote text-muted">{`Last run: ${ran.value}`}</text>}
      <CommandPalette
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="Search commands"
        placeholder="Type a command or a place"
        groups={groups}
        onSelect={id => ran.set(id)}
        emptyLabel="No commands match"
        data-testid="palette"
      />
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return <Button label="Search" startIcon="search" size="sm" onClick={unused} />;
}

/** The playground's props that are not plain values. */
export function playground() {
  return { groups, onSelect: unused } satisfies Partial<CommandPaletteProps>;
}
