import { spec } from '@lingxia/test'

// lingxia dev --background
// lxdev test tests/
// open test-results/lxdev/<run>/report.html

spec('home greets by name', async (t) => {
  await t.app.nav.relaunch({ page: 'home' })

  await t.step('type a name and tap greet', async () => {
    const page = t.app.page
    await page.testId('home-name').fill('Ada')
    await page.testId('home-greet').click()
    await t.expect(page.testId('home-greeting')).toBeVisible()
  })
})
