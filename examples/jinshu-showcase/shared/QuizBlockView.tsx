import { cx, useLocal } from '@lingxia/view';
import { Badge } from '@liulidesign/core/Badge';
import type { BlockProps } from '@liulidesign/jinshu/schema';
import type { quizBlock } from './fixtures';

/** Interactive custom extension block rendered via `<JinshuView customBlocks={{ 'x-demo-quiz': QuizBlockView }}>`. */
export function QuizBlockView({ block, emit }: BlockProps<typeof quizBlock>) {
  const chosen = useLocal<number | null>(`quiz-${block.id}`, null);
  const isAnswered = chosen.value !== null;
  const isCorrect = chosen.value === block.attrs.answerIndex;

  return (
    <view className="rounded-container border border-line bg-surface p-4 gap-3">
      <view className="flex-row items-center justify-between gap-2">
        <Badge label="随堂测验" tone="accent" size="sm" />
        {isAnswered && (
          <Badge label={isCorrect ? '回答正确' : '再想想'} tone={isCorrect ? 'success' : 'danger'} size="sm" />
        )}
      </view>
      <text className="text-body font-medium text-text">{block.attrs.question}</text>
      <view className="gap-2">
        {block.attrs.options.map((opt, idx) => {
          const picked = chosen.value === idx;
          const correctOpt = isAnswered && idx === block.attrs.answerIndex;
          return (
            <button
              key={`${block.id}-opt-${idx}`}
              onClick={() => {
                chosen.set(idx);
                emit('quiz-answer', {
                  selectedIndex: idx,
                  correct: idx === block.attrs.answerIndex,
                });
              }}
              className={cx(
                'min-h-control-lg justify-center rounded-control border px-3.5 py-2.5 text-start transition-colors duration-fast ease-standard focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
                correctOpt
                  ? 'border-success bg-success-soft'
                  : picked
                    ? 'border-danger bg-danger-soft'
                    : 'border-line-strong bg-surface-sunken hover:bg-text/4 active:bg-text/7',
              )}
            >
              <text className="text-footnote text-text">{`${String.fromCharCode(65 + idx)}. ${opt}`}</text>
            </button>
          );
        })}
      </view>
      {isAnswered && (
        <view className="rounded-control bg-info-soft p-3">
          <text className="text-footnote text-text">{block.attrs.explanation}</text>
        </view>
      )}
    </view>
  );
}
