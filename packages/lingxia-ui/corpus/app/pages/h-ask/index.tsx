// h-app pages/ask (views/view.tsx, components/messages.tsx, composer.tsx), ported construct for
// construct: the header with its router pill, the transcript (welcome line, user bubbles, the
// assistant's text, a notice, a question with option buttons, the thinking line) and the
// composer. `ask-shimmer` moves a background-position, which the View cannot animate: the
// thinking line keeps its gradient text without the sweep.
import { useLocal, type View } from '@lingxia/ui';
import '../../theme/heights.css';
import { Button, Icon, cn } from '../../shared/ui';
import type { IconName } from '../../shared/icons';

type Message =
  | { id: string; role: 'user'; text: string }
  | { id: string; role: 'assistant'; text?: string; notice?: 'rateLimited' | 'sendFailed'; question?: { state: 'waiting' | 'answered'; options: { id: string; label: string }[]; chosen?: string } };
type Data = { userName: string; routerName: string; title: string | null; messages: Message[]; thinking: boolean };

const t = {
  via: 'via', welcomeTitle: 'Hi {name}, I’m your network assistant', welcomeBlurb: 'Ask about Wi‑Fi, devices, speed or who is online. I can check your router and make changes with you.',
  thinking: 'Thinking…', composerPlaceholder: 'Ask about your network', sendLabel: 'Send', conversations: 'Conversations', newConversation: 'New conversation',
  rateLimited: 'You’re asking quickly — give me a moment and try again.', sendFailed: 'That message didn’t send. Check your connection and try again.',
  answered: 'You chose {label}',
};
const NOTICE: Record<'rateLimited' | 'sendFailed', { icon: IconName; tone: string }> = {
  rateLimited: { icon: 'Clock', tone: 'bg-warning-subtle text-warning-subtle-foreground' },
  sendFailed: { icon: 'WifiOff', tone: 'bg-destructive-subtle text-destructive-subtle-foreground' },
};

function Avatar({ show, busy = false }: { show: boolean; busy?: boolean }) {
  if (!show) return <view accessibilityHidden className="w-7 shrink-0" />;
  return (
    <view accessibilityHidden className={`brand-gradient mt-0.5 grid size-7 shrink-0 place-items-center rounded-full text-brand-foreground ${busy ? 'ask-avatar-busy' : ''}`}>
      <Icon name="Sparkles" size={13} />
    </view>
  );
}

function AssistantTurn({ message, first }: { message: Extract<Message, { role: 'assistant' }>; first: boolean }) {
  return (
    <row className={`ask-enter items-start gap-2.5 ${first ? '' : '-mt-1.5'}`} testId="assistant-message">
      <Avatar show={first} />
      <column className="min-w-0 flex-1 items-start gap-2">
        {message.notice ? (
          <row className={`max-w-full items-start gap-2 rounded-lg px-3 py-2 ${NOTICE[message.notice].tone}`} testId={`assistant-notice-${message.notice}`}>
            <Icon name={NOTICE[message.notice].icon} size={15} className="mt-0.5" />
            <text dir="auto" className="min-w-0 shrink text-supporting">{t[message.notice]}</text>
          </row>
        ) : message.text ? (
          <text dir="auto" className="selectable max-w-full whitespace-pre-wrap break-words pt-0.5 text-body leading-relaxed text-foreground" testId="assistant-text">{message.text}</text>
        ) : null}
        {message.question?.state === 'answered' ? (
          <row className="items-center gap-1.5" testId="question-answered">
            <Icon name="Check" size={14} className="text-muted-foreground" />
            <text dir="auto" className="text-supporting text-muted-foreground">{t.answered.replace('{label}', message.question.options.find(o => o.id === message.question!.chosen)?.label ?? '')}</text>
          </row>
        ) : message.question ? (
          <row className="flex-wrap gap-2" testId="question-options">
            {message.question.options.map(option => (
              <Button key={option.id} size="sm" variant="outline" className="rounded-full" onClick={() => {}} testId={`question-option-${option.id}`}>
                <text dir="auto" className="text-control-small">{option.label}</text>
              </Button>
            ))}
          </row>
        ) : null}
      </column>
    </row>
  );
}

function Composer() {
  const draft = useLocal('draft', '');
  const ready = draft.value.trim().length > 0;
  return (
    <row className="focus-ring-within items-end gap-1.5 rounded-[1.5rem] border border-border bg-field py-1.5 pe-1.5 ps-4 shadow-[var(--card-shadow)]">
      <textarea bind={draft} rows={1} maxRows={5} enterKeyHint="send" placeholder={t.composerPlaceholder} accessibilityLabel={t.composerPlaceholder} testId="composer"
        className="min-w-0 flex-1 self-center border-0 bg-transparent py-1.5 text-body text-foreground placeholder:text-muted-foreground" />
      <Button size="icon" disabled={!ready} onClick={() => draft.set('')} accessibilityLabel={t.sendLabel} className="size-9 rounded-full" testId="send">
        <Icon name="ArrowUp" size={18} strokeWidth={2.4} />
      </Button>
    </row>
  );
}

export function render({ data }: View<object, Data>) {
  const last = data.messages[data.messages.length - 1];
  return (
    <page className="density-compact flex flex-col bg-background text-body text-foreground">
      <column className="relative mx-auto min-h-0 w-full max-w-md flex-1 flex-col">
        <row className="items-center gap-2 px-gutter pt-2">
          <Button variant="ghost" size="icon" className="rounded-full" accessibilityLabel={t.conversations} onClick={() => {}} testId="conversations-open">
            <Icon name="History" size={19} />
          </Button>
          <row className="min-w-0 flex-1 justify-center">
            {data.title ? (
              <text role="heading" headingLevel={2} dir="auto" className="truncate text-supporting font-medium text-foreground" testId="conversation-current-title">{data.title}</text>
            ) : (
              <row className="items-center gap-1.5 rounded-full bg-secondary px-2.5 py-1">
                <Icon name="Router" size={12} strokeWidth={2.4} className="text-muted-foreground" />
                <text className="text-caption text-muted-foreground">{`${t.via} ${data.routerName}`}</text>
              </row>
            )}
          </row>
          <Button variant="ghost" size="icon" className="rounded-full" accessibilityLabel={t.newConversation} onClick={() => {}} testId="conversation-start">
            <Icon name="MessageSquarePlus" size={20} />
          </Button>
        </row>
        <scroll className="ask-fade-top min-h-0 flex-1 overscroll-contain px-gutter">
          <column className="min-h-full flex-col pb-4 pt-3">
            <column className="mt-auto">
              <row className="mb-5 items-start gap-2.5 border-b border-separator/60 pb-4">
                <Avatar show />
                <column className="min-w-0 shrink">
                  <text className="text-supporting font-medium text-foreground">{t.welcomeTitle.replace('{name}', data.userName)}</text>
                  <text className="mt-0.5 text-caption text-muted-foreground">{t.welcomeBlurb}</text>
                </column>
              </row>
              <column className="flex flex-col gap-5">
                {data.messages.map((message, index) => message.role === 'user' ? (
                  <row key={message.id} className="ask-enter justify-end" testId="user-message">
                    <text dir="auto" className="selectable max-w-[82%] whitespace-pre-wrap break-words rounded-[1.25rem] rounded-ee-md bg-primary px-4 py-2.5 text-primary-foreground">{message.text}</text>
                  </row>
                ) : (
                  <AssistantTurn key={message.id} message={message} first={data.messages[index - 1]?.role !== 'assistant'} />
                ))}
                {data.thinking ? (
                  <row className={`ask-enter items-center gap-2.5 ${last?.role !== 'assistant' ? '' : '-mt-1.5'}`} role="status" testId="thinking">
                    <Avatar show={last?.role !== 'assistant'} busy />
                    <text className="bg-linear-to-r from-muted-foreground via-foreground to-muted-foreground bg-clip-text text-transparent text-supporting font-medium">{t.thinking}</text>
                  </row>
                ) : null}
              </column>
            </column>
          </column>
        </scroll>
        <column className="ask-dock relative px-gutter pb-[calc(max(0.75rem,env(safe-area-inset-bottom))_+_var(--lx-page-chrome-bottom-inset,0px))] pt-2">
          <Composer />
        </column>
      </column>
    </page>
  );
}
