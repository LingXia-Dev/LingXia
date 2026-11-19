import { useLocal } from '@lingxia/view';
import { Said, unused } from '../example';
import { Button } from '../Button/Button';
import { IconButton } from '../IconButton/IconButton';
import { ButtonGroup } from './ButtonGroup';

/** Actions on one thing sit together, the main one last. */
export function Actions() {
  const outcome = useLocal('outcome', 'Draft');
  return (
    <view className="gap-3">
      <ButtonGroup label="Draft actions" data-testid="actions">
        <Button label="Discard" variant="ghost" onClick={() => outcome.set('Discarded')} data-testid="discard" />
        <Button label="Save" onClick={() => outcome.set('Saved')} data-testid="save" />
        <Button label="Publish" variant="primary" onClick={() => outcome.set('Published')} data-testid="publish" />
      </ButtonGroup>
      <text role="status" data-testid="outcome" className="text-footnote text-muted">{outcome.value}</text>
    </view>
  );
}

/** Attached, for actions of one kind: the buttons share edges and the group is one shape. */
export function Attached() {
  return (
    <Said>
      {say => (
        <ButtonGroup label="History" isAttached data-testid="history">
          <Button label="Undo" startIcon="undo" onClick={() => say('Undid the last change')} data-testid="undo" />
          <Button label="Redo" startIcon="redo" onClick={() => say('Redid the last change')} data-testid="redo" />
          <Button label="Reset" onClick={() => say('Reset')} data-testid="reset" />
        </ButtonGroup>
      )}
    </Said>
  );
}

export function IconsAttached() {
  return (
    <Said>
      {say => (
        <ButtonGroup label="Zoom" isAttached variant="secondary" data-testid="zoom">
          <IconButton label="Zoom out" icon="remove" onClick={() => say('Zoomed out')} data-testid="zoom-out" />
          <IconButton label="Zoom in" icon="add" onClick={() => say('Zoomed in')} />
          <IconButton label="Fit to screen" icon="expand" onClick={() => say('Fitted to the screen')} />
        </ButtonGroup>
      )}
    </Said>
  );
}

export function Sizes() {
  return (
    <Said>
      {say => (
        <view className="gap-4">
          <ButtonGroup label="Small" isAttached size="sm" data-testid="small">
            <Button label="Day" onClick={() => say('Showed the day')} data-testid="small-day" />
            <Button label="Week" onClick={() => say('Showed the week')} />
            <Button label="Month" onClick={() => say('Showed the month')} />
          </ButtonGroup>
          <ButtonGroup label="Large" isAttached size="lg">
            <Button label="Day" onClick={() => say('Showed the day')} data-testid="large-day" />
            <Button label="Week" onClick={() => say('Showed the week')} />
            <Button label="Month" onClick={() => say('Showed the month')} />
          </ButtonGroup>
        </view>
      )}
    </Said>
  );
}

export function StateDisabled() {
  return (
    <ButtonGroup label="History" isAttached isDisabled>
      <Button label="Undo" startIcon="undo" onClick={unused} data-testid="disabled-undo" />
      <Button label="Redo" startIcon="redo" onClick={unused} />
    </ButtonGroup>
  );
}

/** A toolbar over content: icon buttons in a quiet group, a labelled action apart from it. */
export function Toolbar() {
  return (
    <Said>
      {say => (
        <view className="flex-row flex-wrap items-center justify-between gap-3">
          <ButtonGroup label="Message actions" size="sm" data-testid="message">
            <IconButton label="Reply" icon="message" onClick={() => say('Replied')} />
            <IconButton label="Forward" icon="send" onClick={() => say('Forwarded')} />
            <IconButton label="Delete" icon="delete" onClick={() => say('Deleted the message')} />
          </ButtonGroup>
          <Button label="Archive" size="sm" startIcon="folder" onClick={() => say('Archived the message')} />
        </view>
      )}
    </Said>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <ButtonGroup label="History" isAttached size="sm">
      <Button label="Undo" onClick={unused} />
      <Button label="Redo" onClick={unused} />
    </ButtonGroup>
  );
}
