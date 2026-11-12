import { useLocal, useView, type View } from '@lingxia/view';
import { Button } from './Button';

export function Variants() {
  return (
    <view className="flex-row flex-wrap gap-2">
      <Button label="Save changes" variant="primary" data-testid="primary" />
      <Button label="Preview" />
      <Button label="Skip" variant="ghost" />
      <Button label="Delete draft" variant="destructive" />
    </view>
  );
}

export function Sizes() {
  return (
    <view className="flex-row flex-wrap items-center gap-2">
      <Button label="Small" size="sm" />
      <Button label="Medium" />
      <Button label="Large" size="lg" />
    </view>
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
    <view className="gap-2">
      <Button
        label="Continue"
        variant="primary"
        width="fill"
        isDisabled={!agreed.value}
        disabledMessage="Accept the terms first"
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
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <view className="flex-row gap-2">
      <Button label="Cancel" size="sm" />
      <Button label="Save" variant="primary" size="sm" />
    </view>
  );
}

/** The page's hero: the one main action beside a quiet one. */
export function Hero() {
  return (
    <view className="flex-row gap-2">
      <Button label="Discard" variant="ghost" />
      <Button label="Save changes" variant="primary" />
    </view>
  );
}

export function StateDefault() {
  return <Button label="Continue" variant="primary" width="content" />;
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
    <view className="flex-row gap-2">
      <Button label="Save" variant="primary" />
      <Button label="Preview" />
      <Button label="Skip" variant="ghost" />
    </view>
  );
}

export function VariantDestructive() {
  return <Button label="Delete draft" variant="destructive" width="content" />;
}

/** A quiet way out beside a form's main action: accent text at regular weight, still a button. */
export function VariantLink() {
  return <Button label="Forgot password?" variant="link" width="content" data-testid="forgot" />;
}
