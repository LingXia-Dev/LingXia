import { useLocal } from '@lingxia/view';
import { MessageBubble } from '../MessageBubble/MessageBubble';
import { Avatar } from '../Avatar/Avatar';
import { unused } from '../example';
import { Citation, CitationGroup, type CitationSource } from './Citation';

const sources: CitationSource[] = [
  { id: 'ferry', title: 'Ferry timetable, summer', site: 'ferries.example.com', excerpt: 'Saturday sailings: 09:30, 14:10 and 16:40. Crossing time about 40 minutes.' },
  { id: 'forecast', title: 'Coast forecast for Saturday', site: 'weather.example.com', excerpt: 'Dry until mid-afternoon; rain from about four, heavier in the evening.' },
  { id: 'notes', title: 'Trip notes', excerpt: 'Tent stays in the car if it looks like rain.' },
];

/** An assistant's answer with three sources: a mark after each claim, the sources as pills under it. */
export function Answer() {
  const opened = useLocal('');
  return (
    <view className="gap-2">
      <MessageBubble avatar={<Avatar name="Assistant" initials="A" size="sm" isDecorative />} author="Assistant">
        <CitationGroup sources={sources} onOpen={source => opened.set(source.title)} data-testid="answer">
          <text className="text-body">
            Take the 14:10 ferry: it crosses in about 40 minutes<Citation source="ferry" /> and lands before the rain starts around four<Citation source="forecast" />. Leave the tent in the car<Citation source="notes" />.
          </text>
        </CitationGroup>
      </MessageBubble>
      {opened.value !== '' && <text className="text-footnote text-muted" data-testid="opened">{`Opened ${opened.value}`}</text>}
    </view>
  );
}

/** One source, cited twice. */
export function Specimen() {
  return (
    <CitationGroup sources={sources.slice(0, 1)} onOpen={unused}>
      <text className="text-body">Boats leave at 09:30 and 14:10<Citation source="ferry" />, about 40 minutes across<Citation source="ferry" />.</text>
    </CitationGroup>
  );
}
