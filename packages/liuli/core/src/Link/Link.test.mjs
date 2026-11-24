import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';
import { textOf } from '@lingxia/view/testing';

test('InSentence: links are runs of their sentence, underlined in the accent colour, and call the app', async () => {
  const screen = await renderExample('Link/Link', 'InSentence');
  const terms = screen.getByTestId('terms');
  assert.deepEqual([terms.tag, textOf(terms), terms.props.color, terms.props.textDecorationLine], ['link', 'terms of service', 'accent', 'underline']);
  screen.press(terms);
  screen.getByText('Opened: terms');
});

test('Disabled: unavailable, in the faint colour, and takes no press', async () => {
  const link = (await renderExample('Link/Link', 'Disabled')).getByTestId('resend');
  assert.deepEqual([link.props.enabled, link.props.color], [0, 'text-faint']);
});

test('ToPage: with to the link goes to the page itself, a run of its sentence', async () => {
  const link = (await renderExample('Link/Link', 'ToPage')).getByTestId('to-settings');
  assert.deepEqual([link.tag, textOf(link), link.props.color], ['link', 'privacy settings', 'accent']);
  assert.equal(typeof link.props.onClick, 'number', 'the View presses it to the page');
});
