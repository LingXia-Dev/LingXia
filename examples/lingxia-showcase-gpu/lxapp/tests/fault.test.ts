import { expect, spec } from '@lingxia/test';

// A page whose render throws shows the recovery panel; automation hears why at once.
spec('a page that cannot render answers with its error', { start: { page: 'spec-fault' } }, async (t) => {
  const failure = await t.app.view.testId('anything').boundingBox().then(() => '', (error: unknown) => String(error));
  expect(failure).toContain('E_PAGE_FAILED');
  expect(failure).toContain('spec-fault: this page fails on purpose');
});
