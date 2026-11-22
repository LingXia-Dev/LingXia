import { expect, test } from '@lingxia/test';
import { openDemo } from './open-demo';

// Right opens a folder and then enters it; Left leaves it and then closes it.
test('arrows open a folder, move into it, move out and close it', { covers: ['view:tree-keys'], start: openDemo('tree') }, async (t) => {
  const view = t.app.view;
  const state = view.testId('lab-tree-state');
  const documents = view.testId('lab-tree-documents');
  const invoices = view.testId('lab-tree-invoices');
  await expect(state).toHaveAttribute('data-state', 'open=none');
  // The arrow beside the name is decoration: the folder is found by its name alone.
  await expect(view.getByRole('treeitem', { name: 'Documents' })).toHaveCount(1);
  await documents.press('ArrowLeft');
  await expect(state).toHaveAttribute('data-state', 'open=none');
  await documents.press('ArrowRight');
  await expect(state).toHaveAttribute('data-state', 'open=documents');
  await expect(documents).toBeFocused();
  await documents.press('ArrowRight');
  await expect(invoices).toBeFocused();
  await invoices.press('ArrowDown');
  await expect(view.testId('lab-tree-letters')).toBeFocused();
  await view.testId('lab-tree-letters').press('ArrowLeft');
  await expect(documents).toBeFocused();
  await expect(state).toHaveAttribute('data-state', 'open=documents');
  await documents.press('ArrowLeft');
  await expect(state).toHaveAttribute('data-state', 'open=none');
  await expect(invoices).toHaveCount(0);
  await expect(documents).toBeFocused();
});

// A named list is found by its name.
test('a list takes a name a screen reader says', { covers: ['view:list-name'], start: openDemo('named') }, async (t) => {
  const view = t.app.view;
  await expect(view.css('[aria-label="Messages"]')).toHaveCount(1);
  // A windowed list builds rows only near the viewport.
  await view.testId('lab-named-list').scrollIntoView();
  expect(await view.css('[aria-label="Messages"] span').count()).toBeGreaterThan(0);
});
