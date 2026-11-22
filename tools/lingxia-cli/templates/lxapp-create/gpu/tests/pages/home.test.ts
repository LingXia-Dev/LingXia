import { test, expect } from '@lingxia/test'

// Specs run in the target App/Runner, separate from Logic and Views.
// `start` relaunches the app on a page first. Drive it through t.app.view
// locators; read its Logic data with (await t.app.page()).data().
//
// lingxia dev --background
// lxdev test tests/pages/home.test.ts
// open test-results/<run>/report.html

test('home streams a greeting by name', { start: { page: 'home' } }, async (t) => {
  const view = t.app.view
  await t.step('type a name and tap greet', async () => {
    await view.testId('home-name').fill('Ada')
    await view.testId('home-greet').click()
    await expect(view.testId('home-greeting')).toHaveText('Hello, Ada! Greeting number 1.')
  })
  await t.step('reset the count', async () => {
    await view.testId('home-reset').click()
    await expect(view.testId('home-reset')).toBeHidden()
  })
})
