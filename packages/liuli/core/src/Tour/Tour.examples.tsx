import { useLocal } from '@lingxia/view';
import { Said } from '../example';
import { Button } from '../Button/Button';
import { IconButton } from '../IconButton/IconButton';
import { SearchField } from '../SearchField/SearchField';
import { Tour, type TourStep } from './Tour';

const steps: TourStep[] = [
  { target: 'tour-new', targetLabel: 'The New note button', title: 'Start a note', text: 'A note opens ready to type, and saves as you go.' },
  { target: 'tour-search', targetLabel: 'The search field', title: 'Find anything', text: 'Search reads every note, list and photo.' },
  { target: 'tour-share', targetLabel: 'The Share button', title: 'Share what you choose', text: 'A note is private until you invite someone.', placement: 'top' },
];

/** Three things on a small screen, shown one at a time; the tour can be skipped or stepped back. */
export function Welcome() {
  const step = useLocal<number | null>('step', null);
  const query = useLocal('query', '');
  return (
    <Said>
      {say => (
        <view className="gap-4">
          {/* At large text the field takes its own line rather than shrinking to its magnifier. */}
          <view className="flex-row flex-wrap items-center gap-2">
            <view id="tour-new"><Button label="New note" variant="primary" startIcon="add" onClick={() => say('Created a note')} /></view>
            <view id="tour-search" className="grow basis-40">
              <SearchField label="Search notes" isLabelHidden value={query.value} onChange={query.set} placeholder="Search" clearLabel="Clear" />
            </view>
          </view>
          <view className="flex-row items-center justify-between gap-3">
            <text className="text-body text-text">Trip to the coast</text>
            <view id="tour-share"><IconButton label="Share" icon="share" onClick={() => say('Shared Trip to the coast')} /></view>
          </view>
          <Button label="Show me around" width="content" onClick={() => step.set(0)} data-testid="tour-start" />
          {step.value !== null && <text role="status" className="text-footnote text-muted">{`On step ${step.value + 1}`}</text>}
          <Tour
            steps={steps}
            value={step.value}
            onChange={next => step.set(next)}
            nextLabel="Next"
            backLabel="Back"
            skipLabel="Skip"
            doneLabel="Done"
            stepLabel={(at, count) => `${at + 1} of ${count}`}
            data-testid="tour"
          />
        </view>
      )}
    </Said>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <view className="w-32 gap-1 rounded-control bg-text p-2">
      <text className="text-caption font-semibold text-surface">Start a note</text>
      <text className="text-caption text-surface">1 of 3</text>
    </view>
  );
}
