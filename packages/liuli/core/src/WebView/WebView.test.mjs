import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

test('ExpandableSheet: renders inline <webview> with corner expand button and shares webSession into the 70% sheet', async () => {
  const screen = await renderExample('WebView/WebView', 'ExpandableSheet');
  const inline = screen.getByTestId('sheet-web');
  assert.equal(inline.tag, 'webview');
  assert.equal(inline.props.accessibilityLabel, 'Delivery area map');
  const session = inline.props.webSession;
  assert.match(session, /^name:/);

  const expand = screen.getByTestId('sheet-web-expand');
  assert.equal(expand.props.accessibilityLabel, 'Expand map');
  screen.press(expand);

  const sheet = screen.getByTestId('sheet-web-sheet');
  assert.equal(sheet.props.role, 'dialog');
  const expanded = screen.getByTestId('sheet-web-expanded');
  assert.equal(expanded.tag, 'webview');
  assert.equal(expanded.props.webSession, session);

  screen.press(screen.getByTestId('sheet-web-close'));
  assert.equal(screen.queryAll(node => node.props.testId === 'sheet-web-sheet').length, 0);
});

test('FullScreenWithActions: controlled full-screen expansion renders footer actions and closes on confirm', async () => {
  const screen = await renderExample('WebView/WebView', 'FullScreenWithActions');
  const inline = screen.getByTestId('full-web');
  assert.equal(inline.props.webSession, 'name:liuli-web-picker');

  screen.press(screen.getByTestId('full-web-expand'));
  assert.ok(screen.getByTestId('full-web-sheet'));
  const expanded = screen.getByTestId('full-web-expanded');
  assert.equal(expanded.props.webSession, 'name:liuli-web-picker');

  screen.press(screen.getByTestId('confirm'));
  assert.equal(screen.queryAll(node => node.props.testId === 'full-web-sheet').length, 0);
});
