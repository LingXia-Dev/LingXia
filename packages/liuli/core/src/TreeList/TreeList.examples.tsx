import { useLocal } from '@lingxia/view';
import { TreeList, type TreeNode } from './TreeList';

const folders: TreeNode[] = [
  {
    id: 'notes', label: 'Notes', icon: 'folder', endText: '48',
    children: [
      { id: 'travel', label: 'Travel', icon: 'folder', endText: '12', children: [
        { id: 'coast', label: 'Trip to the coast', icon: 'file' },
        { id: 'lisbon', label: 'Lisbon in March', icon: 'file' },
      ] },
      { id: 'recipes', label: 'Recipes', icon: 'folder', endText: '9', children: [
        { id: 'bread', label: 'Weekend bread', icon: 'file' },
      ] },
      { id: 'reading', label: 'Reading list', icon: 'file' },
    ],
  },
  { id: 'shared', label: 'Shared with me', icon: 'users', endText: '6', children: [
    { id: 'plan', label: 'Q3 plan', icon: 'file' },
  ] },
  { id: 'archive', label: 'Archive', icon: 'folder', isDisabled: true },
];

/** Folders with their notes: a branch opens in place, and the row that was pressed is the current one. */
export function Folders() {
  const current = useLocal<string | null>('current', 'coast');
  return (
    <view className="gap-2">
      <TreeList label="Folders" nodes={folders} value={current.value} onChange={id => current.set(id)} defaultExpanded={['notes', 'travel']} data-testid="folders" />
      <text role="status" className="text-footnote text-muted">{`Current: ${current.value}`}</text>
    </view>
  );
}

const outline: TreeNode[] = [
  { id: 'intro', label: 'Introduction' },
  { id: 'setup', label: 'Setting up', children: [
    { id: 'install', label: 'Install' },
    { id: 'first', label: 'Your first page', children: [{ id: 'layout', label: 'Layout' }, { id: 'state', label: 'State' }] },
  ] },
  { id: 'publish', label: 'Publishing' },
];

/** An outline without icons, opened from outside. */
export function Outline() {
  const current = useLocal<string | null>('section', 'layout');
  const open = useLocal<readonly string[]>('open', ['setup', 'first']);
  return <TreeList label="Contents" nodes={outline} value={current.value} onChange={id => current.set(id)} expanded={open} data-testid="outline" />;
}

/** The home tile. */
export function Specimen() {
  return (
    <TreeList
      label="Sample"
      nodes={[{ id: 'a', label: 'Notes', children: [{ id: 'b', label: 'Travel' }, { id: 'c', label: 'Work' }] }]}
      value="b"
      onChange={() => {}}
      defaultExpanded={['a']}
      className="w-36"
    />
  );
}
