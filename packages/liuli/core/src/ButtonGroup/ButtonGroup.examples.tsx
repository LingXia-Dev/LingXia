import { useLocal } from '@lingxia/view';
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
    <ButtonGroup label="History" isAttached data-testid="history">
      <Button label="Undo" startIcon="undo" data-testid="undo" />
      <Button label="Redo" startIcon="redo" data-testid="redo" />
      <Button label="Reset" data-testid="reset" />
    </ButtonGroup>
  );
}

export function IconsAttached() {
  return (
    <ButtonGroup label="Zoom" isAttached variant="secondary" data-testid="zoom">
      <IconButton label="Zoom out" icon="remove" data-testid="zoom-out" />
      <IconButton label="Zoom in" icon="add" />
      <IconButton label="Fit to screen" icon="expand" />
    </ButtonGroup>
  );
}

export function Sizes() {
  return (
    <view className="gap-4">
      <ButtonGroup label="Small" isAttached size="sm" data-testid="small">
        <Button label="Day" data-testid="small-day" />
        <Button label="Week" />
        <Button label="Month" />
      </ButtonGroup>
      <ButtonGroup label="Large" isAttached size="lg">
        <Button label="Day" data-testid="large-day" />
        <Button label="Week" />
        <Button label="Month" />
      </ButtonGroup>
    </view>
  );
}

export function StateDisabled() {
  return (
    <ButtonGroup label="History" isAttached isDisabled>
      <Button label="Undo" startIcon="undo" data-testid="disabled-undo" />
      <Button label="Redo" startIcon="redo" />
    </ButtonGroup>
  );
}

/** A toolbar over content: icon buttons in a quiet group, a labelled action apart from it. */
export function Toolbar() {
  return (
    <view className="flex-row items-center justify-between gap-3">
      <ButtonGroup label="Message actions" size="sm" data-testid="message">
        <IconButton label="Reply" icon="message" />
        <IconButton label="Forward" icon="send" />
        <IconButton label="Delete" icon="delete" />
      </ButtonGroup>
      <Button label="Archive" size="sm" startIcon="folder" />
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <ButtonGroup label="History" isAttached size="sm">
      <Button label="Undo" />
      <Button label="Redo" />
    </ButtonGroup>
  );
}
