import { useLocal } from '@lingxia/view';
import { Details, DetailsGroup } from './Details';

const surface = 'gap-1 rounded-container border border-text/12 bg-surface px-5 pt-4 pb-1';

/** One disclosure under the text it qualifies: the summary opens and closes the content under it. */
export function Single() {
  return (
    <view className={surface}>
      <text className="text-body text-text">Team is €8 a month for each active member.</text>
      <Details label="What counts as an active member?" data-testid="active">
        <text className="text-body text-muted">Anyone who opened a shared notebook in the last 30 days. Guests with view access are free.</text>
      </Details>
    </view>
  );
}

/** An accordion: opening one row closes the one that was open. */
export function Accordion() {
  return (
    <DetailsGroup isExclusive defaultValue="refunds" data-testid="faq">
      <Details value="refunds" label="Can I get a refund?" data-testid="refunds">
        <text className="text-body text-muted">Within 14 days of a payment, in full, from Billing.</text>
      </Details>
      <Details value="export" label="How do I export my notes?" data-testid="export">
        <text className="text-body text-muted">Settings, then Export: Markdown files in one archive, with attachments.</text>
      </Details>
      <Details value="delete" label="What happens when I delete my account?" data-testid="delete">
        <text className="text-body text-muted">Your notes are removed from every device after 30 days. Shared notebooks pass to their next member.</text>
      </Details>
    </DetailsGroup>
  );
}

/** Sections of a settings page: an icon and a second line say what is inside; one cannot be opened. */
export function Sections() {
  const open = useLocal<string | null>('open', 'notifications');
  return (
    <DetailsGroup isExclusive label="Settings" value={open.value} onChange={open.set} data-testid="sections">
      <Details value="notifications" label="Notifications" description="Mentions and the weekly digest" startIcon="notifications" data-testid="section-notifications">
        <text className="text-body text-muted">You are told about mentions at once and get a digest on Mondays.</text>
      </Details>
      <Details value="privacy" label="Privacy" description="Who can see your profile" startIcon="lock" data-testid="section-privacy">
        <text className="text-body text-muted">Only members of your workspace can see your profile.</text>
      </Details>
      <Details value="billing" label="Billing" description="Managed by your workspace owner" startIcon="payment" isDisabled data-testid="section-billing">
        <text className="text-body text-muted">Ask the owner for an invoice.</text>
      </Details>
    </DetailsGroup>
  );
}

/** Several rows open at once. */
export function Independent() {
  return (
    <DetailsGroup>
      <Details label="Shipping" isDefaultOpen>
        <text className="text-body text-muted">Free over €40. Two to four working days.</text>
      </Details>
      <Details label="Returns" isDefaultOpen>
        <text className="text-body text-muted">30 days, in the original packaging.</text>
      </Details>
    </DetailsGroup>
  );
}

export function StateClosed() {
  return <DetailsGroup><Details label="Advanced"><text className="text-body text-muted">Proxy and cache.</text></Details></DetailsGroup>;
}

export function StateOpen() {
  return <DetailsGroup><Details label="Advanced" isDefaultOpen><text className="text-body text-muted">Proxy and cache.</text></Details></DetailsGroup>;
}

/** The home tile. */
export function Specimen() {
  return (
    <DetailsGroup className="w-40">
      <Details label="Shipping" isDefaultOpen><text className="text-footnote text-muted">Free over €40.</text></Details>
      <Details label="Returns"><text className="text-footnote text-muted">30 days.</text></Details>
    </DetailsGroup>
  );
}
