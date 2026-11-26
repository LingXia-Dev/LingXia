import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { described, renderExample, renderPage } from '../testing/harness.mjs';

const dialogs = screen => screen.queryAll(node => node.tag === 'dialog');
// The scrim fades in from nothing and back out to it on the panel's own timing.
const presence = node => { const { durationMs, easing } = JSON.parse(node.props.transition); return { durationMs, easing, enters: node.props.startingStyle !== undefined, leaves: node.props.exitStyle !== undefined }; };
const timing = node => { const { durationMs, easing } = JSON.parse(node.props.transition); return { durationMs, easing, enters: true, leaves: true }; };

test('Confirm: the scrim arrives and leaves with the dialog, on its timing', async () => {
  const screen = await renderExample('Dialog/Dialog', 'Confirm');
  screen.press(screen.getByTestId('confirm-open'));
  assert.deepEqual(presence(screen.getByTestId('confirm-dialog-backdrop')), timing(screen.getByTestId('confirm-dialog')));
});

test('Confirm: closed renders nothing; open, an alert dialog named by its title and described by its text, answered by its actions with no close button', async () => {
  const screen = await renderExample('Dialog/Dialog', 'Confirm');
  assert.equal(dialogs(screen).length, 0);
  screen.press(screen.getByTestId('confirm-open'));
  const dialog = screen.getByTestId('confirm-dialog');
  assert.equal(dialog.props.accessibilityLabel, 'Delete this note?');
  assert.equal(dialog.props.role, 'alertdialog');
  assert.equal(screen.queryAll(node => node.tag === 'button' && node.props.accessibilityLabel === 'Close').length, 0);
  assert.deepEqual(described(screen, dialog), ['It leaves every device you are signed in on.']);
  const heading = screen.getByText('Delete this note?');
  assert.equal(heading.props.role, 'heading');
  assert.equal(textOf(screen.getByText('It leaves every device you are signed in on.')), 'It leaves every device you are signed in on.');
  screen.press(screen.getByTestId('confirm'));
  assert.equal(dialogs(screen).length, 0);
  assert.equal(textOf(screen.getByTestId('outcome')), 'Deleted');
});

test('Form: the close button is named "Close" by default and closes; Save waits for a name', async () => {
  const screen = await renderExample('Dialog/Dialog', 'Form');
  screen.press(screen.getByTestId('rename-open'));
  assert.equal(screen.getByTestId('rename-dialog').props.role, 'dialog');
  assert.equal(screen.getByTestId('rename-dialog').props.ariaDescribedBy, undefined);
  assert.equal(screen.getByTestId('save').props.onClick, undefined, 'disabled while empty');
  screen.type(screen.getByTestId('name'), 'Recipes');
  screen.press(screen.getByTestId('save'));
  assert.equal(dialogs(screen).length, 0);
  screen.press(screen.getByTestId('rename-open'));
  const close = screen.queryAll(node => node.tag === 'button' && node.props.accessibilityLabel === 'Close');
  assert.equal(close.length, 1);
  screen.press(close[0]);
  assert.equal(dialogs(screen).length, 0);
});

test('Full: covers the screen with square corners and no floating shadow; the body takes the room left', async () => {
  const screen = await renderExample('Dialog/Dialog', 'Full');
  screen.press(screen.getByTestId('full-open'));
  const dialog = screen.getByTestId('full-dialog');
  assert.deepEqual([dialog.props.widthPercent, dialog.props.heightPercent, dialog.props.radius, dialog.props.boxShadow, dialog.props.background], [100, 100, 0, undefined, 'canvas']);
  assert.equal(dialog.children.find(node => node.tag === 'scroll').props.grow, 1);
});

test('Hero: the picture is drawn only, and the close button over it still closes', async () => {
  const screen = await renderExample('Dialog/Dialog', 'Hero');
  screen.press(screen.getByTestId('hero-open'));
  const dialog = screen.getByTestId('hero-dialog');
  assert.equal(dialog.props.accessibilityLabel, 'Shared notebooks');
  const [media] = dialog.children;
  assert.ok(media.children[0].props.accessibilityHidden, 'the picture is not read');
  const close = media.children[1];
  assert.equal(close.props.accessibilityLabel, 'Close');
  assert.equal(close.props.background, 'media');
  screen.press(close);
  assert.equal(screen.queryAll(node => node.props.testId === 'hero-dialog').length, 0);
});

test('Parts and the tile: the drawn panel\'s secondary action takes the overlay\'s surface, as in a dialog', async () => {
  for (const [name, label] of [['Parts', 'Cancel'], ['Specimen', 'Keep']]) {
    const screen = await renderExample('Dialog/Dialog', name);
    const button = screen.queryAll(node => node.tag === 'button' && textOf(node) === label)[0];
    assert.equal(button.props.background, 'surface-overlay', name);
  }
});

const shown = (screen, id) => screen.queryAll(node => node.testId === id).length;

test('Confirm: an alert closes on Escape and back, never on a backdrop tap', async () => {
  const screen = await renderExample('Dialog/Dialog', 'Confirm');
  screen.press(screen.getByTestId('confirm-open'));
  screen.press(screen.getByTestId('confirm-dialog-backdrop'));
  assert.equal(shown(screen, 'confirm-dialog'), 1, 'a backdrop tap does nothing');
  assert.equal(screen.escape(), true);
  assert.equal(shown(screen, 'confirm-dialog'), 0);
});

// A Logic action the test settles by hand.
const deferred = () => { let settle; const promise = new Promise((resolve, reject) => { settle = { resolve, reject }; }); return { promise, ...settle }; };
const turn = () => new Promise(resolve => setImmediate(resolve));

test('Alert: Cancel comes first; while the action runs the button loads and nothing closes it; a failure stays in the dialog', async () => {
  let pending;
  const screen = await renderExample('Dialog/Dialog', 'Alert', { actions: { publish: () => (pending = deferred()).promise } });
  screen.press(screen.getByTestId('alert-open'));
  const answers = screen.queryAll(node => node.tag === 'button').map(node => textOf(node)).filter(text => text === 'Cancel' || text === 'Delete');
  assert.deepEqual(answers, ['Cancel', 'Delete']);
  screen.press(screen.getByTestId('alert-confirm'));
  await turn();
  screen.escape();
  assert.equal(shown(screen, 'alert'), 1, 'Escape waits too');
  assert.equal(screen.getByTestId('alert-cancel').props.onClick, undefined, 'cancel waits');
  pending.resolve();
  await turn(); await turn();
  screen.rerender({});
  assert.equal(shown(screen, 'alert'), 1, 'a failure keeps it open');
  const failed = screen.getByTestId('alert-failed');
  assert.equal(failed.props.role, 'alert');
  assert.equal(textOf(failed), "That didn't work. Try again.");
  screen.press(screen.getByTestId('alert-cancel'));
  assert.equal(shown(screen, 'alert'), 0);
  screen.toggle(screen.getByTestId('alert-offline'), false);
  screen.press(screen.getByTestId('alert-open'));
  assert.equal(shown(screen, 'alert-failed'), 0, 'reopened without the old failure');
  screen.press(screen.getByTestId('alert-confirm'));
  await turn();
  pending.resolve();
  await turn(); await turn();
  screen.rerender({});
  assert.equal(shown(screen, 'alert'), 0, 'success closes it');
});

test('AlertDialog: an action that fails after the dialog has gone settles quietly', async () => {
  let pending;
  const screen = await renderPage(`
    import { useLocal, useView } from '@lingxia/view';
    import { AlertDialog } from '@liulidesign/core/AlertDialog';
    export function render() {
      const { actions } = useView();
      const shown = useLocal('shown', true);
      return <page>
        <button data-testid="drop" onClick={() => shown.set(false)}><text>Drop</text></button>
        {shown.value && <AlertDialog isOpen onOpenChange={() => {}} label="Delete?" confirmLabel="Delete" confirmAction={() => actions.publish()} data-testid="gone" />}
      </page>;
    }
  `, { actions: { publish: () => (pending = deferred()).promise } });
  screen.press(screen.getByTestId('gone-confirm'));
  await turn();
  screen.press(screen.getByTestId('drop'));
  const errors = [];
  const original = console.error;
  console.error = (...parts) => errors.push(parts.join(' '));
  try {
    pending.reject(Error('offline'));
    await turn(); await turn();
  } finally {
    console.error = original;
  }
  assert.equal(shown(screen, 'gone'), 0);
  assert.ok(errors.every(line => !line.includes('UI_LOCAL')), errors.join('\n'));
});

test('a close button carries <id>-close, over media too, and an AlertDialog keeps its data-* props', async () => {
  const screen = await renderPage(`
    import { Dialog, AlertDialog } from '@liulidesign/core/Dialog';
    export function render() {
      return (
        <page>
          <Dialog isOpen onOpenChange={() => {}} label="Plain" hasCloseButton data-testid="plain"><text>Body</text></Dialog>
          <AlertDialog isOpen onOpenChange={() => {}} label="Delete?" confirmLabel="Delete" confirmAction={async () => {}} data-testid="alert" data-kind="delete" />
        </page>
      );
    }`);
  assert.equal(screen.getByTestId('plain-close').tag, 'button');
  assert.deepEqual(JSON.parse(screen.getByTestId('alert').props.dataAttributes), { kind: 'delete' });
});
