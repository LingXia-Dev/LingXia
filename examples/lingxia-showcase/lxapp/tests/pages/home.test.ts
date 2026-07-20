import { expect, test } from '@rongjs/test';

test('greets through real page input and the Logic bridge', async () => {
  const app = lx.automation().lxapp();
  await app.nav.relaunch({ page: 'home' });
  await app.page.waitFor({ page: 'home', css: '[data-testid="home-page"]' });

  const name = `Gate ${Date.now()}`;
  await app.page.fill({ page: 'home', css: '[data-testid="home-name"]', text: name });
  await app.page.click({ page: 'home', css: '[data-testid="home-greet"]' });

  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    const greeting = await app.page.query({
      page: 'home',
      css: '[data-testid="home-greeting"]',
      full: true,
    });
    if (greeting.exists && greeting.text.includes(name)) {
      expect(greeting.text).toContain(name);
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(`Timed out waiting for greeting: ${name}`);
});
