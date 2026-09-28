import { spec, expect } from '@lingxia/test'

// Specs run in the target App/Runner, separate from Logic and WebViews.
// `start` relaunches the app on a page first; drive it through t.app.view locators.
//
// lingxia dev --background
// lxdev test tests/pages/home.test.ts

spec('home shows the native shell title', { start: { page: 'home' } }, async (t) => {
  await expect(t.app.view.testId('home-title')).toBeVisible()
})
