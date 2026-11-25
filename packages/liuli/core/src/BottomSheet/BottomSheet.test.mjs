import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample, renderPage } from '../testing/harness.mjs';

const sheet = (screen, testId) => screen.queryAll(node => node.testId === testId)[0];
const drag = node => JSON.parse(node.props.drag);
// The scrim fades in from nothing and back out to it on the panel's own timing.
const presence = node => { const { durationMs, easing } = JSON.parse(node.props.transition); return { durationMs, easing, enters: node.props.startingStyle !== undefined, leaves: node.props.exitStyle !== undefined }; };
const timing = node => { const { durationMs, easing } = JSON.parse(node.props.transition); return { durationMs, easing, enters: true, leaves: true }; };

test('MoveTo: the scrim arrives and leaves with the sheet, on its timing', async () => {
  const screen = await renderExample('BottomSheet/BottomSheet', 'MoveTo');
  screen.press(screen.getByTestId('move-open'));
  assert.deepEqual(presence(sheet(screen, 'move-backdrop')), timing(sheet(screen, 'move')));
});

test('MoveTo: a named dialog on the bottom edge, as tall as its content, that drags off', async () => {
  const screen = await renderExample('BottomSheet/BottomSheet', 'MoveTo');
  assert.equal(sheet(screen, 'move'), undefined);
  screen.press(screen.getByTestId('move-open'));
  const panel = sheet(screen, 'move');
  assert.equal(panel.props.accessibilityLabel, 'Move to');
  assert.deepEqual(drag(panel), { axis: 'y', points: [0, '100%'], index: 0 });
  screen.press(screen.getByTestId('folder-Travel'));
  assert.equal(sheet(screen, 'move'), undefined);
  assert.equal(textOf(screen.getByTestId('folder')), 'In Travel');
});

test('MoveTo: coming to rest off screen closes it, and the next open starts at the top again', async () => {
  const screen = await renderExample('BottomSheet/BottomSheet', 'MoveTo');
  screen.press(screen.getByTestId('move-open'));
  screen.drag(sheet(screen, 'move'), { phase: 'settle', index: 1, hidden: true });
  assert.equal(sheet(screen, 'move'), undefined);
  screen.press(screen.getByTestId('move-open'));
  assert.equal(drag(sheet(screen, 'move')).index, 0);
});

test('Comments: an expandable sheet opens halfway, drags up, and reopens halfway after the close button', async () => {
  const screen = await renderExample('BottomSheet/BottomSheet', 'Comments');
  screen.press(screen.getByTestId('comments-open'));
  assert.deepEqual(drag(sheet(screen, 'comments')), { axis: 'y', points: [0, '45%', '100%'], index: 1 });
  screen.drag(sheet(screen, 'comments'), { phase: 'settle', index: 0 });
  assert.equal(drag(sheet(screen, 'comments')).index, 0);
  const close = screen.getByTestId('comments-close');
  assert.equal(close.props.accessibilityLabel, 'Close');
  screen.press(close);
  assert.equal(sheet(screen, 'comments'), undefined);
  screen.press(screen.getByTestId('comments-open'));
  assert.equal(drag(sheet(screen, 'comments')).index, 1);
});

test('initialDetent full: an expandable sheet opens at its full detent, and reopens there', async () => {
  const screen = await renderPage(`
    import { useLocal } from '@lingxia/view';
    import { BottomSheet } from '@liulidesign/core/BottomSheet';
    export function render() {
      const open = useLocal('open', true);
      return <page><button data-testid="again" onClick={() => open.set(true)}><text>Open</text></button><BottomSheet isOpen={open.value} onOpenChange={open.set} label="Reply" detents="expandable" initialDetent="full" hasCloseButton data-testid="reply"><text>Body</text></BottomSheet></page>;
    }
  `);
  assert.deepEqual([drag(sheet(screen, 'reply')).points, drag(sheet(screen, 'reply')).index], [[0, '45%', '100%'], 0]);
  screen.drag(sheet(screen, 'reply'), { phase: 'settle', index: 1 });
  screen.press(screen.getByTestId('reply-close'));
  screen.press(screen.getByTestId('again'));
  assert.equal(drag(sheet(screen, 'reply')).index, 0);
});

test('an expandable sheet the page closes itself reopens halfway', async () => {
  const screen = await renderPage(`
    import { useLocal } from '@lingxia/view';
    import { BottomSheet } from '@liulidesign/core/BottomSheet';
    export function render() {
      const open = useLocal('open', true);
      return <page><button data-testid="again" onClick={() => open.set(true)}><text>Open</text></button><BottomSheet isOpen={open.value} onOpenChange={open.set} label="Reply" detents="expandable" data-testid="reply"><button data-testid="done" onClick={() => open.set(false)}><text>Done</text></button></BottomSheet></page>;
    }
  `);
  screen.drag(sheet(screen, 'reply'), { phase: 'settle', index: 0 });
  screen.press(screen.getByTestId('done'));
  screen.press(screen.getByTestId('again'));
  assert.equal(drag(sheet(screen, 'reply')).index, 1);
});

test('Required: with dismissOn none the sheet has no off-screen point and only its buttons close it', async () => {
  const screen = await renderExample('BottomSheet/BottomSheet', 'Required');
  screen.press(screen.getByTestId('leave-open'));
  assert.deepEqual(drag(sheet(screen, 'leave')).points, [0]);
  screen.press(screen.getByTestId('stay'));
  assert.equal(sheet(screen, 'leave'), undefined);
  assert.equal(textOf(screen.getByTestId('answer')), 'Stayed');
});

test('the panel runs to the bottom edge and its content clears the home indicator: its 20 plus the inset the layer adds', async () => {
  const edges = { top: 0, right: 0, bottom: 34, left: 0 };
  const screen = await renderExample('BottomSheet/BottomSheet', 'MoveTo', { env: { insets: edges, safeArea: edges } });
  screen.press(screen.getByTestId('move-open'));
  assert.equal(sheet(screen, 'move').props.paddingBottom, 54);
});

const phone = { env: { sizeClass: 'compact', viewport: { width: 393, height: 852 } } };
const wide = { env: { sizeClass: 'regular', viewport: { width: 1024, height: 768 } } };

test('Required: on a phone the actions share the footer equally, the main one last; wider they keep their widths', async () => {
  for (const [form, shared] of [[phone, true], [wide, false]]) {
    const screen = await renderExample('BottomSheet/BottomSheet', 'Required', form);
    screen.press(screen.getByTestId('leave-open'));
    const [stay, leave] = [screen.getByTestId('stay'), screen.getByTestId('leave-confirm')];
    assert.equal(stay.parent, leave.parent);
    assert.equal(stay.parent.children.indexOf(leave), stay.parent.children.length - 1, 'the main action is last');
    for (const button of [stay, leave]) assert.equal(button.props.grow === 1, shared, `${form.env.sizeClass}: ${button.testId}`);
  }
});

test('MoveTo: the header action sits between the title and the close button', async () => {
  const screen = await renderExample('BottomSheet/BottomSheet', 'MoveTo');
  screen.press(screen.getByTestId('move-open'));
  const action = screen.getByTestId('move-new-folder');
  assert.equal(action.props.accessibilityLabel, 'New folder');
  const header = screen.getByText('Move to').parent;
  let at = action;
  while (at.parent !== header) at = at.parent;
  assert.equal(header.children.indexOf(at), 1);
});

test('Comments: a hairline at the top while content is under the header, at the bottom while more is under the edge; the content does not move', async () => {
  const screen = await renderExample('BottomSheet/BottomSheet', 'Comments');
  screen.press(screen.getByTestId('comments-open'));
  const body = () => screen.queryAll(node => node.tag === 'scroll' && node.props.onScrollEdge !== undefined)[0];
  const lines = () => {
    const { borderTopWidth: top, borderBottomWidth: bottom, marginTop, marginBottom, borderColor } = body().props;
    if (top) assert.equal(marginTop, -top, 'the line takes back its point at the top');
    if (bottom) assert.equal(marginBottom, -bottom, 'and at the bottom');
    if (top || bottom) assert.equal(borderColor, 'line', 'the line role, which high contrast strengthens');
    return [top ? 'top' : '', bottom ? 'bottom' : ''].filter(Boolean);
  };
  assert.deepEqual(lines(), [], 'nothing before the body reports its edges, so a body that fits never flashes a line');
  screen.fire(body(), 'onScrollEdge', JSON.stringify({ edge: 'start', at: true }));
  assert.deepEqual(lines(), ['bottom'], 'a long body reports only its start: the end is unreached');
  const at = (start, end) => {
    screen.fire(body(), 'onScrollEdge', JSON.stringify({ edge: 'start', at: start }));
    screen.fire(body(), 'onScrollEdge', JSON.stringify({ edge: 'end', at: end }));
    return lines();
  };
  assert.deepEqual([at(true, true), at(true, false), at(false, true), at(false, false)], [[], ['bottom'], ['top'], ['top', 'bottom']]);
});

test('Comments: a sheet opened again starts its lines afresh, not where the last opening left them', { skip: 'waits for the View to report scroll edges after it resets an offset itself, as on mount' }, async () => {
  const screen = await renderExample('BottomSheet/BottomSheet', 'Comments');
  const body = () => screen.queryAll(node => node.tag === 'scroll' && node.props.onScrollEdge !== undefined)[0];
  const edge = (name, at) => screen.fire(body(), 'onScrollEdge', JSON.stringify({ edge: name, at }));
  const bottom = () => Boolean(body().props.borderBottomWidth);
  screen.press(screen.getByTestId('comments-open'));
  edge('start', true); edge('start', false); edge('end', true);
  assert.equal(bottom(), false, 'scrolled to the end: nothing more under the footer');
  screen.press(screen.getByTestId('comments-close'));
  screen.press(screen.getByTestId('comments-open'));
  edge('start', true);
  assert.equal(bottom(), true, 'opened again at the top of a long body: the line over the footer is back');
});
