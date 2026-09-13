import { spec } from '@lingxia/test'

// Specs run in the target App/Runner, separate from Logic and WebViews.
// Drive the UI with t.app; use t.app.eval for Logic-only checks.
//
// lingxia dev --background
// lxdev test tests/pages/home.test.ts

spec('home shows the native shell title', async (t) => {
  await t.app.nav.relaunch({ page: 'home' })
  await t.expect(t.app.page.testId('home-title')).toBeVisible()
})
