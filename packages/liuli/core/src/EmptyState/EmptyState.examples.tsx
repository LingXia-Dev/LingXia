import { useLocal, type VectorPath } from '@lingxia/view';
import { Said } from '../example';
import { Button } from '../Button/Button';
import { Card } from '../Card/Card';
import { EmptyState } from './EmptyState';

const notebook: VectorPath[] = [
  { d: 'M6 3.5h11a1.5 1.5 0 0 1 1.5 1.5v14a1.5 1.5 0 0 1-1.5 1.5H6zM6 3.5v17M9.5 8h5.5M9.5 11.5h5.5', fill: 'none', stroke: 'currentColor', strokeWidth: 1.5, strokeLinecap: 'round', strokeLinejoin: 'round' },
];
const search: VectorPath[] = [
  { d: 'M10.5 4a6.5 6.5 0 1 0 0 13a6.5 6.5 0 1 0 0-13zM15.5 15.5L20 20', fill: 'none', stroke: 'currentColor', strokeWidth: 1.5, strokeLinecap: 'round' },
];

/** A first-run screen: what will be here, and the one action that starts it. */
export function FirstRun() {
  const created = useLocal('created', false);
  return (
    <EmptyState
      title={created.value ? 'Notebook created' : 'No notebooks yet'}
      description="Notebooks keep related notes together, and sync to your other devices."
      media={<vector aria-hidden paths={notebook} viewBox={[0, 0, 24, 24]} className="size-full" />}
      actions={<Button label="New notebook" variant="primary" onClick={() => created.set(true)} data-testid="create" />}
      data-testid="first-run"
    />
  );
}

/** No results: say what was searched, and offer a way back. */
export function NoResults() {
  const query = useLocal('query', 'invoice 2019');
  return (
    <Said>
      {say => (
        <EmptyState
          title="No matching notes"
          description={`Nothing matches "${query.value}". Check the spelling, or search all folders.`}
          media={<vector aria-hidden paths={search} viewBox={[0, 0, 24, 24]} className="size-full" />}
          actions={
            <>
              <Button label="Clear search" variant="ghost" onClick={() => query.set('')} />
              <Button label="Search all folders" onClick={() => say('Searched all folders')} />
            </>
          }
          data-testid="no-results"
        />
      )}
    </Said>
  );
}

/** Small, inside a card: no picture, one line. */
export function InCard() {
  return (
    <Card>
      <text role="heading" headingLevel={3} className="text-callout font-semibold text-text">Shared with you</text>
      <EmptyState size="sm" headingLevel={4} title="Nothing shared yet" description="Notes others share with you appear here." data-testid="in-card" />
    </Card>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <EmptyState
      size="sm"
      title="No notes"
      media={<vector aria-hidden paths={notebook} viewBox={[0, 0, 24, 24]} className="size-full" />}
      className="w-32 py-2"
    />
  );
}

export function Sizes() {
  return (
    <view className="gap-4">
      <Card padding="none">
        <EmptyState size="sm" title="Small" description="Inside a card or a list." />
      </Card>
      <Card padding="none">
        <EmptyState title="Medium" description="Filling a page or a panel." />
      </Card>
    </view>
  );
}

export function StateTitleOnly() {
  return <EmptyState size="sm" title="No messages" />;
}

export function StateWithPicture() {
  return <EmptyState size="sm" title="No notes" description="Start one from the + button." media={<vector aria-hidden paths={notebook} viewBox={[0, 0, 24, 24]} className="size-full" />} />;
}

export function StateWithAction() {
  return <Said>{say => <EmptyState size="sm" title="Offline" description="Notes you open while online stay here." actions={<Button label="Try again" size="sm" onClick={() => say('Retried')} />} />}</Said>;
}
