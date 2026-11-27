import { expect, test } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '@lingxia/test/showcase/helpers/app.js';

// The native page calls notes.* routes in native/src/notes.rs: a call, an app error, a stream.
test('native routes: call, app error, and stream result', {
  id: 'NATIVE-ROUTES-001',
  app: SHOWCASE_APP_ID,
  start: { page: 'native' },
}, async (t) => {
  const view = t.app.view;
  await t.step('an empty title fails with the app code', async () => {
    await view.testId('native-add').click();
    await expect(view.testId('native-status')).toHaveText('EMPTY_TITLE: a note needs a title');
  });
  await t.step('a call returns the new note', async () => {
    await view.testId('native-title').fill('Buy milk');
    await view.testId('native-add').click();
    await expect(view.testId('native-status')).toContainText('Added note');
  });
  await t.step('a missing note fails with NOT_FOUND and its id', async () => {
    await view.testId('native-missing').click();
    await expect(view.testId('native-status')).toContainText('NOT_FOUND: no note');
  });
  await t.step('the stream reports progress and returns the text', async () => {
    await view.testId('native-export').click();
    await expect(view.testId('native-status')).toHaveText('Export finished');
    await expect(view.testId('native-exported')).toContainText('Buy milk');
  });
});
