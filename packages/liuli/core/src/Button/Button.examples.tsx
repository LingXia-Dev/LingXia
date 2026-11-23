import { useLocal, useView, type View } from '@lingxia/view';
import { Said, unused } from '../example';
import { Button } from './Button';
import { Item } from '../Item/Item';
import { Card } from '../Card/Card';

export function Variants() {
  return (
    <Said>
      {say => (
        <view className="flex-row flex-wrap gap-2">
          <Button label="Save changes" variant="primary" onClick={() => say('Saved changes')} data-testid="primary" />
          <Button label="Preview" onClick={() => say('Opened the preview')} />
          <Button label="Turn off internet" variant="tonal" startIcon="pause" onClick={() => say('Turned off the internet')} data-testid="tonal" />
          <Button label="Skip" variant="ghost" onClick={() => say('Skipped')} />
          <Button label="Delete draft" variant="destructive" onClick={() => say('Deleted the draft')} />
        </view>
      )}
    </Said>
  );
}

export function Sizes() {
  return (
    <Said>
      {say => (
        <view className="flex-row flex-wrap items-center gap-2">
          <Button label="Small" size="sm" onClick={() => say('Chose Small')} />
          <Button label="Medium" onClick={() => say('Chose Medium')} />
          <Button label="Large" size="lg" onClick={() => say('Chose Large')} />
        </view>
      )}
    </Said>
  );
}

type Actions = { publish(): Promise<void> };

/** `clickAction` takes a Logic action as it is; the button shows loading until it settles. */
export function AsyncAction() {
  const { actions } = useView() as View<Actions>;
  return <Button label="Publish" variant="primary" width="content" clickAction={() => actions.publish()} data-testid="publish" />;
}

/** The reason is read after the label while disabled. Pass translated copy; Liuli ships none. */
export function DisabledWithReason() {
  const agreed = useLocal('agreed', false);
  return (
    <Said>
      {say => (
        <view className="gap-2">
          <Button
            label="Continue"
            variant="primary"
            width="fill"
            isDisabled={!agreed.value}
            disabledMessage="Accept the terms first"
            onClick={() => say('Continued')}
            data-testid="continue"
          />
          <Button
            label={agreed.value ? 'Withdraw consent' : 'Accept the terms'}
            variant="ghost"
            width="content"
            onClick={() => agreed.set(!agreed.value)}
            data-testid="toggle-terms"
          />
        </view>
      )}
    </Said>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <view className="flex-row flex-wrap gap-2">
      <Button label="Cancel" size="sm" onClick={unused} />
      <Button label="Save" variant="primary" size="sm" onClick={unused} />
    </view>
  );
}

/** The page's hero: the one main action beside a quiet one. */
export function Hero() {
  return (
    <Said>
      {say => (
        <view className="flex-row flex-wrap gap-2">
          <Button label="Discard" variant="ghost" onClick={() => say('Discarded changes')} />
          <Button label="Save changes" variant="primary" onClick={() => say('Saved changes')} />
        </view>
      )}
    </Said>
  );
}

export function StateDefault() {
  return <Said>{say => <Button label="Continue" variant="primary" width="content" onClick={() => say('Continued')} />}</Said>;
}

export function StateDisabled() {
  return <Button label="Continue" variant="primary" width="content" isDisabled disabledMessage="Accept the terms first" />;
}

export function StateBusy() {
  return <Button label="Publishing" variant="primary" width="content" isLoading />;
}

export function StateSecondaryDisabled() {
  return <Button label="Preview" width="content" isDisabled />;
}

export function VariantsEveryday() {
  return (
    <Said>
      {say => (
        <view className="flex-row flex-wrap gap-2">
          <Button label="Save" variant="primary" onClick={() => say('Saved')} />
          <Button label="Preview" onClick={() => say('Opened the preview')} />
          <Button label="Skip" variant="ghost" onClick={() => say('Skipped')} />
        </view>
      )}
    </Said>
  );
}

export function VariantDestructive() {
  return <Said>{say => <Button label="Delete draft" variant="destructive" width="content" onClick={() => say('Deleted the draft')} />}</Said>;
}

/** A quiet way out beside a form's main action: accent text at regular weight, still a button. */
export function VariantLink() {
  return <Said>{say => <Button label="Forgot password?" variant="link" width="content" onClick={() => say('Sent a password reset link')} data-testid="forgot" />}</Said>;
}

/** An icon before the label says what kind of action it is; one after it says where the button leads. */
export function WithIcons() {
  return (
    <Said>
      {say => (
        <view className="flex-row flex-wrap gap-3">
          <Button label="New note" variant="primary" startIcon="add" onClick={() => say('Created a note')} data-testid="new-note" />
          <Button label="Share" startIcon="share" onClick={() => say('Shared')} />
          <Button label="Continue" endIcon="arrow-end" onClick={() => say('Continued')} data-testid="continue-on" />
          <Button label="Delete" variant="destructive" startIcon="delete" onClick={() => say('Deleted')} />
        </view>
      )}
    </Said>
  );
}

const notes = [
  ['Trip to the coast', 'Edited yesterday'],
  ['Reading list', 'Edited on Monday'],
  ['Ideas for the garden', 'Edited last week'],
  ['Recipes to try', 'Edited on 2 November'],
  ['Gift list', 'Edited on 28 October'],
  ['Books of the year', 'Edited on 14 October'],
] as const;

/** Floating: the screen's one main action, inside the frame, over a list that scrolls under it. */
export function Floating() {
  return (
    <Said>
      {say => (
        <view className="relative h-72 overflow-hidden rounded-container border border-text/12 bg-surface">
          <scroll className="h-full">
            <view className="divide-y divide-text/8">
              {notes.map(([label, edited]) => <Item key={label} label={label} description={edited} />)}
            </view>
          </scroll>
          <view className="absolute bottom-4 end-4">
            <Button label="New note" variant="primary" size="lg" startIcon="add" isFloating onClick={() => say('Created a note')} data-testid="floating" />
          </view>
        </view>
      )}
    </Said>
  );
}

/** A card's own action, filled but quiet beside the page's one primary button; off while it cannot run. */
export function VariantTonal() {
  return (
    <Said>
      {say => (
        <view className="gap-3">
          <Card>
            <view className="gap-3">
              <text className="text-body">Emily's devices are online.</text>
              <view className="flex-row flex-wrap gap-2">
                <Button label="Turn off internet" variant="tonal" startIcon="pause" onClick={() => say('Turned off the internet')} />
                <Button label="Study time" variant="tonal" isDisabled disabledMessage="No schedule is set" />
              </view>
            </view>
          </Card>
        </view>
      )}
    </Said>
  );
}
