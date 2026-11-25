import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

const texts = (screen, role) => screen.queryAll(node => node.props.role === role && node.tag === 'text').map(textOf);
// What the live texts of a role say now, ignoring the cleared ones.
const live = (screen, role) => texts(screen, role).filter(text => text !== '');
const shown = screen => screen.queryAll(node => node.testId === 'toast');
const message = screen => textOf(shown(screen)[0].children.find(node => node.tag === 'text'));

test('Archive: a toast shows and is announced; Undo runs and closes it', async () => {
  const screen = await renderExample('Toast/Toast', 'Archive');
  assert.equal(shown(screen).length, 0);
  assert.deepEqual(live(screen, 'status'), []);
  screen.press(screen.getByTestId('archive-q3-plan'));
  assert.equal(shown(screen).length, 1);
  assert.equal(message(screen), '"Q3 plan" archived');
  assert.deepEqual(live(screen, 'status'), ['"Q3 plan" archived']);
  assert.deepEqual(live(screen, 'alert'), []);
  screen.press(screen.getByTestId('toast-action'));
  assert.equal(shown(screen).length, 0);
  screen.getByTestId('archive-q3-plan');
});

test('Archive: the dismiss button is named and closes the toast', async () => {
  const screen = await renderExample('Toast/Toast', 'Archive');
  screen.press(screen.getByTestId('archive-reading-list'));
  const dismiss = screen.getByTestId('toast-dismiss');
  assert.equal(dismiss.props.accessibilityLabel, 'Dismiss');
  screen.press(dismiss);
  assert.equal(shown(screen).length, 0);
  assert.equal(screen.queryAll(node => node.testId === 'archive-reading-list').length, 0);
});

test('Archive: a swipe to either side closes it; a drag back to rest keeps it', async () => {
  const screen = await renderExample('Toast/Toast', 'Archive');
  screen.press(screen.getByTestId('archive-q3-plan'));
  const card = () => screen.queryAll(node => node.props.drag !== undefined)[0];
  assert.equal(card().props.accessibilityLabel, '"Q3 plan" archived');
  screen.drag(card(), { phase: 'start' });
  screen.drag(card(), { phase: 'end', index: 1, velocity: 0 });
  assert.equal(shown(screen).length, 1);
  screen.drag(card(), { phase: 'end', index: 2, velocity: 900 });
  assert.equal(shown(screen).length, 0);
});

test('Tones: toasts queue and show one at a time; a danger toast is an alert', async () => {
  const screen = await renderExample('Toast/Toast', 'Tones');
  screen.press(screen.getByTestId('show-success'));
  screen.press(screen.getByTestId('show-danger'));
  assert.equal(shown(screen).length, 1);
  assert.equal(message(screen), 'Changes saved');
  assert.deepEqual(live(screen, 'status'), ['Changes saved']);
  screen.press(screen.getByTestId('toast-dismiss'));
  assert.equal(message(screen), 'Could not save the note');
  assert.deepEqual(live(screen, 'status'), []);
  assert.deepEqual(live(screen, 'alert'), ['Could not save the note']);
  screen.press(screen.getByTestId('toast-dismiss'));
  assert.equal(shown(screen).length, 0);
});

test('Tones: the same message twice in a row is announced twice, by a live text that was cleared first', async () => {
  const screen = await renderExample('Toast/Toast', 'Tones');
  screen.press(screen.getByTestId('show-success'));
  screen.press(screen.getByTestId('show-success'));
  const first = texts(screen, 'status');
  assert.deepEqual(first.filter(text => text !== ''), ['Changes saved']);
  screen.press(screen.getByTestId('toast-dismiss'));
  const second = texts(screen, 'status');
  assert.equal(message(screen), 'Changes saved');
  const speaking = second.findIndex(text => text === 'Changes saved');
  assert.ok(speaking >= 0);
  assert.equal(first[speaking], '', 'the live text now speaking was empty in the frame before, so its text changed');
});

test('Progress: a toast with the same id updates in place instead of queueing', async () => {
  const screen = await renderExample('Toast/Toast', 'Progress');
  screen.press(screen.getByTestId('upload'));
  screen.press(screen.getByTestId('upload'));
  assert.equal(shown(screen).length, 1);
  assert.equal(message(screen), 'Uploaded 2 of 3');
  screen.press(screen.getByTestId('upload'));
  assert.equal(message(screen), 'All 3 photos uploaded');
  screen.press(screen.getByTestId('toast-dismiss'));
  assert.equal(shown(screen).length, 0);
});

test('like the reference, the toast sits in the bottom end corner on any screen, as wide as the region up to 400 pt', async () => {
  for (const pointer of ['coarse', 'fine']) {
    const screen = await renderExample('Toast/Toast', 'Tones', { env: { pointer } });
    screen.press(screen.getByTestId('show-info'));
    assert.deepEqual([region(screen).props.justify, region(screen).props.align, region(screen).props.paddingBottom], ['end', 'end', 16], pointer);
    // The top-layer box is the corner, not the page: on a touch screen the bottom row, with a pointer the card and its gap.
    const { insetTop, insetBottom, insetInlineStart, insetInlineEnd, insetLeft, insetRight } = region(screen).props;
    assert.deepEqual([insetTop, insetBottom, insetInlineStart, insetInlineEnd], [undefined, 0, undefined, 0], pointer);
    assert.deepEqual([insetLeft, insetRight], pointer === 'fine' ? [undefined, undefined] : [0, 0], pointer);
    const card = screen.getByTestId('toast').parent;
    assert.deepEqual([card.props.widthPercent ?? card.props.width, card.props.maxWidth], [100, 400], pointer);
  }
});

test('position moves the toast to another corner, from its own edge; inset keeps an edge clear for the page\'s own bar', async () => {
  const { renderPage } = await import('../testing/harness.mjs');
  const page = (props) => `
    import { Button } from '@liulidesign/core/Button';
    import { ToastRegion, useToast } from '@liulidesign/core/Toast';
    function Save() { const toast = useToast(); return <Button label="Save" onClick={() => toast.show({ message: 'Saved' })} data-testid="save" />; }
    export function render() { return <page><ToastRegion ${props}><Save /></ToastRegion></page>; }
  `;
  const top = await renderPage(page(`position="topStart"`));
  top.press(top.getByTestId('save'));
  assert.deepEqual([region(top).props.justify, region(top).props.align], ['start', 'start']);
  assert.match(region(top).props.startingStyle, /\\"translate\\":\[0,-8\]/, 'it moves in from above');
  const clear = await renderPage(page(`inset={{ bottom: 64, end: 24 }}`));
  clear.press(clear.getByTestId('save'));
  assert.deepEqual([region(clear).props.marginBottom, region(clear).props.marginInlineEnd, region(clear).props.paddingBottom], [64, 24, 16], 'the page\'s bar as a margin; the gap stays');
  assert.match(region(clear).props.startingStyle, /\\"translate\\":\[0,8\]/, 'at the bottom it rises from below');
});

test('the toast clears the bars and the home indicator, and sits on an open keyboard', async () => {
  const insets = { top: 92, right: 0, bottom: 34, left: 0 };
  const phone = await renderExample('Toast/Toast', 'Tones', { env: { insets } });
  phone.press(phone.getByTestId('show-info'));
  assert.equal(region(phone).props.paddingBottom, 50);
  assert.equal(region(phone).props.paddingTop, 108);
  const typing = await renderExample('Toast/Toast', 'Tones', { env: { insets, keyboard: { height: 300 } } });
  typing.press(typing.getByTestId('show-info'));
  assert.equal(region(typing).props.paddingBottom, 16);
});

const region = screen => screen.queryAll(node => node.props.elementId !== undefined && typeof node.props.justify === 'string')[0];
// The top-layer box the popover lands in carries its wire fields.
const layer = screen => screen.queryAll(node => node.props.topLayer === 2)[0];

test('a toast without an action closes after the region\'s duration; the next one shows', async () => {
  const screen = await renderExample('Toast/Toast', 'Tones');
  screen.press(screen.getByTestId('show-success'));
  screen.press(screen.getByTestId('show-info'));
  assert.equal(layer(screen).props.closeAfter, 5000);
  screen.host.__lingxiaCloseAfter(region(screen).props.elementId);
  screen.rerender();
  assert.equal(message(screen), 'Link copied');
  screen.host.__lingxiaCloseAfter(region(screen).props.elementId);
  screen.rerender();
  assert.equal(shown(screen).length, 0);
});

test('a toast with an action never closes by itself', async () => {
  const screen = await renderExample('Toast/Toast', 'Archive');
  screen.press(screen.getByTestId('archive-q3-plan'));
  assert.equal(layer(screen).props.closeAfter, undefined);
});

test('a toast\'s own duration overrides the region\'s', async () => {
  const screen = await renderExample('Toast/Toast', 'Tones');
  screen.press(screen.getByTestId('show-warning'));
  assert.equal(layer(screen).props.closeAfter, 8000);
});

test('a showing toast leaves the rest of the page pressable: its top-layer box never covers the page', async () => {
  const { renderPage } = await import('../testing/harness.mjs');
  for (const position of ['bottomEnd', 'topStart']) for (const pointer of ['coarse', 'fine']) {
    const screen = await renderPage(`
      import { useLocal } from '@lingxia/view';
      import { Button } from '@liulidesign/core/Button';
      import { ToastRegion, useToast } from '@liulidesign/core/Toast';
      function Page() {
        const toast = useToast();
        const count = useLocal('count', 0);
        return <view>
          <Button label="Save" onClick={() => toast.show({ message: 'Saved', action: { label: 'Undo', onClick: () => {} } })} data-testid="save" />
          <Button label="Count" onClick={() => count.set(count.value + 1)} data-testid="count" />
          <text data-testid="counted">{String(count.value)}</text>
        </view>;
      }
      export function render() { return <page><ToastRegion position="${position}"><Page /></ToastRegion></page>; }
    `, { env: { pointer } });
    screen.press(screen.getByTestId('save'));
    assert.equal(shown(screen).length, 1);
    // A box that met all four edges would sit over the whole page, and a press there would never reach it.
    const props = region(screen).props;
    const edges = ['insetTop', 'insetBottom'].filter(edge => props[edge] === 0).length + (props.insetLeft === 0 && props.insetRight === 0 ? 2 : [props.insetInlineStart, props.insetInlineEnd].filter(edge => edge === 0).length);
    assert.ok(edges < 4, `${position} ${pointer}: the region meets ${edges} edges`);
    screen.press(screen.getByTestId('count'));
    assert.equal(screen.getByTestId('counted').text ?? screen.getByTestId('counted').children?.[0]?.text, '1', `${position} ${pointer}: the press reached the page`);
    assert.equal(shown(screen).length, 1, 'and the undo toast stays');
  }
});
