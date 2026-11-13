import { useLocal } from '@lingxia/view';
import { Banner } from './Banner';

/** A problem with one way out, that the person can also put aside. */
export function Payment() {
  const shown = useLocal(true);
  const updated = useLocal(false);
  if (!shown.value) return <text className="text-footnote text-muted">Dismissed. It comes back on the next visit.</text>;
  return (
    <Banner
      tone="warning"
      title="Your card expires this month"
      message="Update it before 30 June to keep syncing across devices."
      action={{ label: updated.value ? 'Card updated' : 'Update card', onClick: () => updated.set(true) }}
      onDismiss={() => shown.set(false)}
      dismissLabel="Dismiss"
      data-testid="payment"
    />
  );
}

export function Tones() {
  return (
    <view className="gap-3">
      <Banner tone="neutral" title="Read only" message="An admin locked this notebook." />
      <Banner tone="info" title="Sharing is changing" message="From July, shared notebooks need a Team plan." />
      <Banner tone="success" title="All notes synced" />
      <Banner tone="warning" title="Storage almost full" message="You have used 9.6 of 10 GB." />
      <Banner tone="danger" title="Sync stopped" message="We could not reach the server. Your changes are kept on this device." />
    </view>
  );
}

/** At the top of a list it is about, before the content. */
export function OverList() {
  const retried = useLocal(false);
  return (
    <view className="gap-3">
      <Banner
        tone="danger"
        title={retried.value ? 'Still offline' : 'Sync stopped'}
        message="Changes are kept on this device until the connection returns."
        action={{ label: 'Retry now', onClick: () => retried.set(true) }}
        data-testid="sync"
      />
      <view className="overflow-hidden rounded-container border border-text/10 bg-surface divide-y divide-text/8">
        {['Q3 plan', 'Reading list', 'Interview notes'].map(note => (
          <view key={note} className="px-4 py-3"><text className="text-body text-text">{note}</text></view>
        ))}
      </view>
    </view>
  );
}

export function StateTitleOnly() {
  return <Banner tone="success" title="Saved" />;
}

export function StateWithMessage() {
  return <Banner tone="neutral" title="Locked" message="An admin locked it." />;
}

export function StateWithAction() {
  return <Banner tone="danger" title="Not sent" action={{ label: 'Retry', onClick: () => {} }} />;
}

export function StateDismissible() {
  return <Banner title="New" onDismiss={() => {}} dismissLabel="Dismiss" />;
}

/** The home tile. */
export function Specimen() {
  return <Banner tone="success" title="All synced" className="w-44" />;
}
