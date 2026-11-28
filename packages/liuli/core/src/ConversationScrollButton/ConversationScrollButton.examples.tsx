import { List, useLocal } from '@lingxia/view';
import { Button } from '../Button/Button';
import { MessageBubble } from '../MessageBubble/MessageBubble';
import { ConversationScrollButton, useConversationScroll } from './ConversationScrollButton';

const opening = Array.from({ length: 24 }, (_, index) => ({ id: `m${index + 1}`, mine: index % 3 === 2, text: index % 3 === 2 ? `Noted, item ${index + 1}.` : `Packing list item ${index + 1}: ${['tent', 'stove', 'rain jackets', 'torch'][index % 4]}` }));

/** A conversation longer than its box: scroll up and the button appears; messages that arrive meanwhile are counted on it. */
export function Follow() {
  const messages = useLocal('messages', opening);
  const list = messages.value;
  const scroll = useConversationScroll(list.length);
  const arrive = () => messages.set([...list, { id: `m${list.length + 1}`, mine: false, text: `New message ${list.length + 1}` }]);
  return (
    <view className="gap-3">
      <view className="h-80 overflow-hidden rounded-container border border-line bg-canvas">
        <List
          aria-label="Messages"
          items={list}
          getKey={message => message.id}
          anchor={scroll.anchor}
          onScrollEdge={scroll.onScrollEdge}
          renderItem={message => <MessageBubble side={message.mine ? 'end' : 'start'} text={message.text} className="pb-1" data-testid={`message-${message.id}`} />}
          className="grow basis-0 min-h-0 px-3 pt-3"
          data-testid="messages"
        />
        <ConversationScrollButton scroll={scroll} data-testid="latest" />
      </view>
      <Button label="A message arrives" size="sm" onClick={arrive} className="self-start" data-testid="arrive" />
    </view>
  );
}
