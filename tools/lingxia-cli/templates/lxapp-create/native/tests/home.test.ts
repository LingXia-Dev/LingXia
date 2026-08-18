import { spec } from '@lingxia/test'

// lingxia dev --background
// lxdev test tests/

spec('home shows the native shell title', async (t) => {
  await t.app.nav.relaunch({ page: 'home' })
  await t.expect(t.app.page.testId('home-title')).toBeVisible()
})
