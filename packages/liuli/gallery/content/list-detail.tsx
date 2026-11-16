import { useBox, useLocal, useView } from '@lingxia/view';
import { Avatar } from '@liulidesign/core/Avatar';
import { Badge } from '@liulidesign/core/Badge';
import { Button } from '@liulidesign/core/Button';
import { ButtonGroup } from '@liulidesign/core/ButtonGroup';
import { Divider } from '@liulidesign/core/Divider';
import { EmptyState } from '@liulidesign/core/EmptyState';
import { Said } from '@liulidesign/core/example';
import { IconButton } from '@liulidesign/core/IconButton';
import { Item, ItemList } from '@liulidesign/core/Item';
import { Markdown } from '@liulidesign/core/Markdown';
import { MetadataList } from '@liulidesign/core/MetadataList';
import { SearchField } from '@liulidesign/core/SearchField';
import { Section } from '@liulidesign/core/Section';
import { SplitView } from '@liulidesign/core/SplitView';

interface Request {
  id: string;
  from: string;
  team: string;
  subject: string;
  when: string;
  status: 'Submitted' | 'In review' | 'Approved';
  amount: string;
  neededBy: string;
  body: string;
}

const requests: Request[] = [
  {
    id: 'r-218', from: 'Ana Lima', team: 'Design', subject: 'Two more seats for the research tool', when: '09:42', status: 'Submitted', amount: '€16 a month', neededBy: '28 November',
    body: 'We have **two researchers** starting on the 1st and the current plan has no seats left.\n\n- Plan: Team, billed yearly\n- Cost: €16 a month more\n- Needed by: 28 November\n\nI can cover it from the tools budget if that is easier.',
  },
  {
    id: 'r-217', from: 'Kai Chen', team: 'Engineering', subject: 'Conference travel, Lisbon', when: 'Yesterday', status: 'In review', amount: '€1,180', neededBy: '15 December',
    body: 'Three days at the systems conference in March. Flights and hotel come to about **€1,180**.\n\n1. Flights booked by 15 December are a third cheaper\n2. The talk I submitted was accepted\n\n> Finance asked for the agenda; it is attached to the ticket.',
  },
  {
    id: 'r-214', from: 'Mira Patel', team: 'Support', subject: 'Replace the shared headset', when: 'Mon', status: 'Approved', amount: '€89', neededBy: '20 November',
    body: 'The headset at the support desk drops calls. A wired replacement is **€89**.\n\n- [x] Checked the warranty: expired in May\n- [x] Tried another cable\n- [ ] Order placed',
  },
  {
    id: 'r-209', from: 'Sam Ortiz', team: 'Sales', subject: 'Customer dinner, 4 December', when: '8 Nov', status: 'Approved', amount: '€420', neededBy: '4 December',
    body: 'Dinner for six with the Northwind team after the renewal call. Budget asked: **€420**.',
  },
  {
    id: 'r-204', from: 'Tomás Reis', team: 'Engineering', subject: 'Second monitor', when: '5 Nov', status: 'In review', amount: '€310', neededBy: '30 November',
    body: 'A 27-inch monitor for the build station, **€310**. The current one flickers on wake.',
  },
];

const tone = { Submitted: 'accent', 'In review': 'warning', Approved: 'success' } as const;

/** One request, as a row of the list. */
function Row({ request, isUnread, isSelected, onOpen }: { request: Request; isUnread: boolean; isSelected?: boolean; onOpen: () => void }) {
  return (
    <Item
      label={request.subject}
      // An unread request says so in words, for a screen reader, and carries a dot, for the eye.
      description={`${isUnread ? 'Unread · ' : ''}${request.from} · ${request.team}`}
      startContent={<Avatar name={request.from} size="sm" isDecorative />}
      endContent={(
        <view className="items-end gap-1.5">
          <text className="text-footnote text-muted tabular-nums">{request.when}</text>
          <view className={isUnread ? 'size-2 rounded-full bg-accent' : 'size-2'} />
        </view>
      )}
      isSelected={isSelected}
      onClick={onOpen}
      data-testid={`request-${request.id}`}
    />
  );
}

/** The record: who asked, what for, and what can be done about it. */
function Detail({ request, onDecide }: { request: Request; onDecide: (status: Request['status']) => void }) {
  return (
    // Keyed by the request, so one record's line does not stay under the next.
    <Said key={request.id}>
      {say => (
        <view className="gap-5">
          <view className="gap-3">
            <view className="flex-row items-center gap-2">
              <Badge label={request.status} tone={tone[request.status]} />
              <view className="grow" />
              <ButtonGroup label="More actions">
                <IconButton label="Share" icon="share" size="sm" onClick={() => say(`Shared ${request.subject}`)} />
                <IconButton label="Print" icon="print" size="sm" onClick={() => say(`Printed ${request.subject}`)} />
              </ButtonGroup>
            </view>
            <text role="heading" headingLevel={2} className="text-title font-semibold tracking-tight text-text">{request.subject}</text>
            <view className="flex-row items-center gap-3">
              <Avatar name={request.from} isDecorative />
              <view className="gap-0.5">
                <text className="text-body font-medium text-text">{request.from}</text>
                <text className="text-footnote text-muted">{request.team}</text>
              </view>
            </view>
          </view>
          <Markdown source={request.body} topHeadingLevel={3} taskLabels={{ done: 'Done', open: 'Not done' }} />
          <Divider />
          <Section title="Details" headingLevel={3}>
            <MetadataList
              label="Request details"
              items={[
                { key: 'reference', label: 'Reference', value: request.id.toUpperCase() },
                { key: 'received', label: 'Received', value: request.when },
                { key: 'amount', label: 'Amount', value: request.amount },
                { key: 'needed', label: 'Needed by', value: request.neededBy },
              ]}
              data-testid="details"
            />
          </Section>
          <view className="flex-row items-center gap-2 pt-1">
            <Button label="Approve" variant="primary" isDisabled={request.status === 'Approved'} onClick={() => onDecide('Approved')} data-testid="approve" />
            <Button label="Ask a question" onClick={() => onDecide('In review')} data-testid="ask" />
          </view>
        </view>
      )}
    </Said>
  );
}

export function Content() {
  const { env } = useView();
  const frame = useBox('frame');
  const query = useLocal('query', '');
  const open = useLocal<string | null>('open', null);
  const decided = useLocal<Record<string, Request['status']>>('decided', {});
  // What has been opened; a new request is unread until then.
  const read = useLocal<readonly string[]>('read', []);
  const isUnread = (request: Request) => request.status === 'Submitted' && !read.value.includes(request.id);
  const show = (id: string) => { open.set(id); if (!read.value.includes(id)) read.set([...read.value, id]); };
  const all = requests.map(request => ({ ...request, status: decided.value[request.id] ?? request.status }));
  const typed = query.value.trim().toLowerCase();
  const shown = all.filter(request => `${request.subject} ${request.from} ${request.team}`.toLowerCase().includes(typed));
  // Two panes need room for a 320 px list and a readable record beside it.
  const isWide = (frame.value?.width ?? (env.sizeClass === 'regular' ? 900 : 328)) >= 680;
  // Side by side something is always open; on a phone the list is the first screen.
  const current = all.find(request => request.id === open.value) ?? (isWide ? shown[0] ?? null : null);
  // The record on show has been read, whether it was pressed or is the first one shown beside the list.
  const isNew = (request: Request) => isUnread(request) && request.id !== current?.id;
  const unread = all.filter(isNew).length;
  const decide = (status: Request['status']) => { if (current) decided.set({ ...decided.value, [current.id]: status }); };

  const list = (selected?: string) => (
    <view className="gap-3">
      <SearchField label="Search requests" value={query.value} onChange={query.set} placeholder="Search" clearLabel="Clear" resultsLabel={`${shown.length} of ${all.length} requests`} isResultsLabelHidden data-testid="search" />
      {shown.length > 0
        ? <ItemList label="Requests">{shown.map(request => <Row key={request.id} request={request} isUnread={isNew(request)} isSelected={selected === undefined ? undefined : request.id === selected} onOpen={() => show(request.id)} />)}</ItemList>
        : <EmptyState title="No requests match" description="Try a name, a team or a word from the subject." size="sm" />}
    </view>
  );

  return (
    <view box={frame} className="gap-4">
      {/* On a phone an open record's back row is the header: the place is named once. */}
      {(isWide || !current) && (
        <view className="flex-row items-center justify-between gap-3">
          <text role="heading" headingLevel={1} className="text-title font-semibold tracking-tight text-text">Requests</text>
          {unread > 0 && <Badge label={`${unread} unread`} tone="accent" />}
        </view>
      )}
      {isWide ? (
        <view className="h-[560px] overflow-hidden rounded-container border border-text/12">
          <SplitView sidebar={list(current?.id)} sidebarLabel="Requests" contentLabel={current?.subject ?? 'Request'} sidebarWidth={340} contentKey={current?.id} contentWidth="prose" data-testid="requests">
            {current ? <Detail request={current} onDecide={decide} /> : <EmptyState title="No request selected" description="Choose a request from the list." />}
          </SplitView>
        </view>
      ) : current ? (
        <view className="gap-4">
          <Button label="Requests" startIcon="chevron-start" variant="ghost" size="sm" width="content" onClick={() => open.set(null)} data-testid="back" />
          <Detail request={current} onDecide={decide} />
        </view>
      ) : list()}
    </view>
  );
}
