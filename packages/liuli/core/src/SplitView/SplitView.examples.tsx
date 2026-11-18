import { useLocal } from '@lingxia/view';
import { unused } from '../example';
import { NavList } from '../NavList/NavList';
import { SplitView, type SplitViewPane } from './SplitView';

const sections = [
  { id: 'mail', items: [
    { value: 'inbox', label: 'Inbox', endText: '12' },
    { value: 'drafts', label: 'Drafts', endText: '3' },
    { value: 'sent', label: 'Sent' },
  ] },
  { id: 'projects', title: 'Projects', items: [
    { value: 'launch', label: 'Spring launch' },
    { value: 'research', label: 'User research' },
  ] },
] as const;
const titles: Record<string, string> = { inbox: 'Inbox', drafts: 'Drafts', sent: 'Sent', launch: 'Spring launch', research: 'User research' };

/** A mail workspace: the sidebar chooses a place, the content shows it at once; on a phone the place opens over the list, with a way back. */
export function Workspace() {
  const current = useLocal<string>('current', 'inbox');
  const pane = useLocal<SplitViewPane>('pane', 'sidebar');
  const title = titles[current.value] ?? '';
  return (
    <view className="h-[360px] overflow-hidden rounded-container border border-text/12">
      <SplitView
        sidebarLabel="Mail"
        contentLabel={title}
        pane={pane.value}
        onBack={() => pane.set('sidebar')}
        sidebarWidth={200}
        contentKey={current.value}
        sidebar={<NavList label="Mailboxes" sections={sections} value={current.value} onChange={next => { current.set(next); pane.set('content'); }} data-testid="workspace-nav" />}
        data-testid="workspace"
      >
        <text role="heading" headingLevel={1} className="text-title font-semibold text-text" data-testid="workspace-title">{title}</text>
        <text className="text-body text-muted">Each pane scrolls on its own. Beside the sidebar the content keeps a gutter from it; on a phone it takes the screen.</text>
      </SplitView>
    </view>
  );
}

/** The tile: a sidebar's rows beside the open page, drawn at the tile's size (SplitView's own padding is for a window). */
export function Specimen() {
  return (
    <view className="w-full flex-row overflow-hidden rounded-control border border-text/12 bg-canvas">
      <view className="w-[84px] p-1 border-e border-text/8">
        <NavList label="Mail" sections={[{ id: 'mail', items: [{ value: 'inbox', label: 'Inbox' }, { value: 'sent', label: 'Sent' }] }]} value="inbox" onChange={unused} />
      </view>
      <view className="grow shrink basis-0 p-2.5 gap-1">
        <text className="text-footnote font-semibold text-text">Inbox</text>
        <text className="text-caption text-muted">12 new</text>
      </view>
    </view>
  );
}
