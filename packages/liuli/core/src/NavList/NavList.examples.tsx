import { useLocal } from '@lingxia/view';
import { unused } from '../example';
import { NavList, type NavListProps } from './NavList';

const workspace = [
  { id: 'mail', items: [
    { value: 'inbox', label: 'Inbox', endText: '12' },
    { value: 'drafts', label: 'Drafts', endText: '3' },
    { value: 'sent', label: 'Sent' },
  ] },
  { id: 'projects', title: 'Projects', items: [
    { value: 'launch', label: 'Spring launch' },
    { value: 'research', label: 'User research' },
    { value: 'archive', label: 'Archive', isDisabled: true },
  ] },
] as const;

/** A sidebar: places first, then a section that closes; the current row is selected. */
export function Sidebar() {
  const current = useLocal<string>('current', 'inbox');
  return (
    <view className="md:max-w-[264px] gap-4">
      <NavList label="Workspace" sections={workspace} value={current.value} onChange={next => current.set(next)} data-testid="workspace" />
      <text className="text-footnote text-muted" data-testid="workspace-current">{`Showing ${current.value}`}</text>
    </view>
  );
}

const library = [
  { id: 'inputs', title: 'Inputs', items: [{ value: 'text-input', label: 'TextInput' }, { value: 'date-input', label: 'DateInput' }] },
  { id: 'feedback', title: 'Feedback', items: [{ value: 'badge', label: 'Badge' }, { value: 'toast', label: 'Toast' }] },
] as const;

/** Only the section in use starts open; each remembers being opened. */
export function Closed() {
  const current = useLocal<string>('current', 'text-input');
  return (
    <view className="md:max-w-[264px]">
      <NavList label="Components" sections={library} value={current.value} onChange={next => current.set(next)} defaultOpenSections={['inputs']} data-testid="library" />
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <view className="w-32">
      <NavList label="Mail" sections={[{ id: 'mail', items: [{ value: 'inbox', label: 'Inbox', endText: '12' }, { value: 'sent', label: 'Sent' }] }]} value="inbox" onChange={unused} />
    </view>
  );
}

/** The playground's props that are not plain values. */
export function playground() {
  return { sections: workspace } satisfies Partial<NavListProps>;
}
