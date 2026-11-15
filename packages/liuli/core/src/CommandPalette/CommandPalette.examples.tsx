import { useLocal } from '@lingxia/view';
import { Button } from '../Button/Button';
import { CommandPalette, type CommandGroup } from './CommandPalette';

const groups: CommandGroup[] = [
  { label: 'Go to', items: [
    { id: 'all', label: 'All notes', icon: 'file', endText: 'G then A' },
    { id: 'shared', label: 'Shared with me', icon: 'users', endText: 'G then S' },
    { id: 'archive', label: 'Archive', icon: 'folder' },
    { id: 'settings', label: 'Settings', icon: 'settings', description: 'Account, appearance, sync' },
  ] },
  { label: 'Actions', items: [
    { id: 'new', label: 'New note', icon: 'add', endText: 'Ctrl N' },
    { id: 'share', label: 'Share this note', icon: 'share' },
    { id: 'export', label: 'Export as PDF', icon: 'upload', isDisabled: true },
  ] },
];

/** Opened from a button here; an app also opens it from its own shortcut. Type to narrow, arrows to move, Return to run. */
export function Commands() {
  const open = useLocal('open', false);
  const ran = useLocal('ran', 'Nothing yet');
  return (
    <view className="items-start gap-2">
      <Button label="Search commands" startIcon="search" onClick={() => open.set(true)} data-testid="palette-open" />
      <text role="status" className="text-footnote text-muted">{`Last run: ${ran.value}`}</text>
      <CommandPalette
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="Search commands"
        placeholder="Type a command or a place"
        groups={groups}
        onSelect={id => ran.set(id)}
        emptyText="No commands match"
        data-testid="palette"
      />
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return <Button label="Search" startIcon="search" size="sm" />;
}
