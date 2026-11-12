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
        onClick={() => agreed.set(!agreed.value)}
        data-testid="toggle-terms"
      />
    </view>
  );
}
