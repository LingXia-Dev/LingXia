import { useLocal } from '@lingxia/view';
import { Badge } from '../Badge/Badge';
import { Said, unused } from '../example';
import { IconButton } from '../IconButton/IconButton';
import { TreeList, type TreeNode, type TreeListProps } from './TreeList';

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
      <TreeList label="Folders" items={folders} bind={current} defaultOpenBranches={['notes', 'travel']} data-testid="folders" />
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
  return <TreeList label="Contents" items={outline} bind={current} openBranches={open.value} onOpenBranchesChange={next => open.set(next)} data-testid="outline" />;
}

/** The outline with guides: a hairline runs down from each open branch beside its children. */
export function Guides() {
  const current = useLocal<string | null>('section', 'state');
  return (
    <TreeList
      label="Contents"
      items={outline}
      bind={current}
      defaultOpenBranches={['setup', 'first']}
      hasLineGuides
      data-testid="guides"
    />
  );
}

/** Rows with a second line, and the row's own content at the end: a status, a button with its own action. */
export function Described() {
  const current = useLocal<string | null>('current', 'cart');
  return (
    <Said>
      {say => {
        const project: TreeNode[] = [
          { id: 'src', label: 'src', icon: 'folder', description: 'The app\'s source', children: [
            { id: 'cart', label: 'cart.ts', icon: 'file', description: 'Edited 2 minutes ago', endContent: <Badge label="Changed" tone="warning" size="sm" /> },
            { id: 'pay', label: 'pay.ts', icon: 'file', description: 'Edited yesterday' },
          ] },
          { id: 'tests', label: 'tests', icon: 'folder', description: '14 files', endContent: <IconButton label="Run the tests" icon="play" size="sm" variant="ghost" onClick={() => say('Ran the tests')} data-testid="run" />, children: [] },
        ];
        return (
          <TreeList label="Project" items={project} bind={current} defaultOpenBranches={['src']} hasLineGuides data-testid="project" />
        );
      }}
    </Said>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <TreeList
      label="Sample"
      items={[{ id: 'a', label: 'Notes', children: [{ id: 'b', label: 'Travel' }] }]}
      value="b"
      onChange={unused}
      defaultOpenBranches={['a']}
      className="w-full"
    />
  );
}

/** The playground's props that are not plain values. */
export function playground() {
  return { items: folders, defaultOpenBranches: ['notes', 'travel'] } satisfies Partial<TreeListProps>;
}
