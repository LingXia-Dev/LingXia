import { spec } from '@lingxia/test'

// Specs run in the target App/Runner, separate from Logic and WebViews.
// Drive the UI with t.app; use t.app.eval for Logic-only checks.
//
// lingxia dev --background
// lxdev test tests/pages/home.test.ts
// open test-results/<run>/report.html

spec('home greets by name', async (t) => {
  await t.app.nav.relaunch({ page: 'home' })

  await t.step('type a name and tap greet', async () => {
    const page = t.app.page
    await page.testId('home-name').fill('Ada')
    await page.testId('home-greet').click()
    await t.expect(page.testId('home-greeting')).toBeVisible()
  })
})
