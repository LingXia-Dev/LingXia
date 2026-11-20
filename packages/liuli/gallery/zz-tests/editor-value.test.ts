import { expect, test } from '@lingxia/test';
import { reveal } from './finger.js';

test('text-area: Send while focused clears the reply on screen and keeps focus', {
  id: 'EDITOR-VALUE-CLEAR-001', start: { page: 'text-area' }, timeout: 60_000,
}, async (t) => {
  const view = t.app.view;
  const reply = view.testId('reply');
  await reveal(view, reply);
  await reply.click();
  await reply.type('On my way');
  await expect(reply).toHaveValue('On my way');
  await reply.press('Enter');
  await expect(view.testId('sent')).toHaveText('On my way');
  await expect(reply).toHaveValue('');
  await expect(reply).toBeFocused();
  // The value is right in the model; the shot shows whether the editor repainted it empty.
  // A focused editor's caret blinks; a steady one lets the page settle.
  await view.environment({ still: true });
  await view.settled();
  await t.attach('reply-after-send.png', await view.screenshot());
});
