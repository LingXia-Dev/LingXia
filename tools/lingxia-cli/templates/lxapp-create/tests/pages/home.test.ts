import { spec } from '@lingxia/test'

// Specs run in the target App/Runner, separate from Logic and WebViews.
// Drive the page through t.app.view locators; read Logic with t.app.logic.
//
// lingxia dev --background
// lxdev test tests/pages/home.test.ts
// open test-results/<run>/report.html

spec('home greets by name', async (t) => {
  await t.app.nav.relaunch({ page: 'home' })

  await t.step('type a name and tap greet', async () => {
    const view = t.app.view
    await view.testId('home-name').fill('Ada')
    await view.testId('home-greet').click()
    await t.expect(view.testId('home-greeting')).toBeVisible()
  })
})
