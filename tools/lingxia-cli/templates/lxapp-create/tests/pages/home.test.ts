import { spec, expect } from '@lingxia/test'

// Specs run in the target App/Runner, separate from Logic and WebViews.
// `start` relaunches the app on a page first. Drive it through t.app.view
// locators; read its Logic data with (await t.app.page()).data().
//
// lingxia dev --background
// lxdev test tests/pages/home.test.ts
// open test-results/<run>/report.html

spec('home greets by name', { start: { page: 'home' } }, async (t) => {
  await t.step('type a name and tap greet', async () => {
    const view = t.app.view
    await view.testId('home-name').fill('Ada')
    await view.testId('home-greet').click()
    await expect(view.testId('home-greeting')).toBeVisible()
  })
})
