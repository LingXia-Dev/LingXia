import { Avatar } from '../Avatar/Avatar';
import { Timestamp } from './Timestamp';

const now = '2026-11-14T10:05';

const activity = [
  { who: 'Ana Lima', what: 'commented on Trip to the coast', when: '2026-11-14T10:05' },
  { who: 'Kai Chen', what: 'shared Reading list with you', when: '2026-11-14T09:42' },
  { who: 'Mira Patel', what: 'edited Ideas for the garden', when: '2026-11-14T07:10' },
  { who: 'Sam Ortiz', what: 'archived Gift list', when: '2026-11-13T18:30' },
  { who: 'Tomás Reis', what: 'joined the workspace', when: '2026-11-02T11:15' },
  { who: 'Ana Lima', what: 'created the workspace', when: '2025-12-19T16:00' },
] as const;

/** An activity list: how long ago for today, then yesterday, then the date. */
export function Activity() {
  return (
    <view className="gap-3">
      {activity.map((entry, index) => (
        <view key={entry.when + entry.who} className="flex-row items-center gap-3">
          <Avatar name={entry.who} size="sm" isDecorative />
          <view className="grow basis-0 min-w-0 gap-0.5">
            <text className="text-body text-text">{`${entry.who} ${entry.what}`}</text>
            <Timestamp value={entry.when} now={now} className="text-footnote text-muted" data-testid={`when-${index}`} />
          </view>
        </view>
      ))}
    </view>
  );
}

/** The same moment in each form. */
export function Formats() {
  const value = '2026-11-13T18:30';
  return (
    <view className="gap-2">
      {(['relative', 'date', 'time', 'date-time'] as const).map(format => (
        <view key={format} className="flex-row items-baseline justify-between gap-4">
          <text className="text-footnote text-muted">{format}</text>
          <Timestamp value={value} now={now} format={format} className="text-body text-text" data-testid={`format-${format}`} />
        </view>
      ))}
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return <Timestamp value="2026-11-14T09:42" now={now} className="text-footnote text-muted" />;
}
