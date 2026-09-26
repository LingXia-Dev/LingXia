import { spec } from '@lingxia/test'

// Specs run in the target App/Runner, separate from Logic and WebViews.
// Drive the page through t.app.view locators; read Logic with t.app.logic.
//
// lingxia dev --background
// lxdev test tests/pages/home.test.ts

spec('home shows the native shell title', async (t) => {
  await t.app.nav.relaunch({ page: 'home' })
  await t.expect(t.app.view.testId('home-title')).toBeVisible()
})
