import { expect, spec } from '@lingxia/test';

// A page whose render throws shows the recovery panel; automation hears why at once.
// Opening it is the call that rejects where the host reports the failure at once; elsewhere the first read does.
spec('a page that cannot render answers with its error', async (t) => {
  const failure = await t.app.nav.to({ page: 'spec-fault' })
    .then(() => t.app.view.testId('anything').boundingBox())
    .then(() => '', (error: unknown) => String(error));
  expect(failure).toContain('E_PAGE_FAILED');
  expect(failure).toContain('spec-fault: this page fails on purpose');
});
