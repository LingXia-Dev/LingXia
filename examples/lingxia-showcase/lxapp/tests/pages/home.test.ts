import { expect, spec } from '@lingxia/test';
import { showcaseApp } from '../helpers/app.js';
import {
  waitForElementAttribute,
  waitForElementEnabled,
  waitForElementText,
} from '../helpers/page.js';

spec('greets through real page input and the Logic bridge', async () => {
  const app = showcaseApp();
  await app.nav.relaunch({ page: 'home' });
  await app.page.waitFor({ page: 'home', css: '[data-testid="home-page"]' });

  const name = `Gate ${Date.now()}`;
  await app.page.fill({ page: 'home', css: '[data-testid="home-name"]', text: name });
  await waitForElementAttribute(
    app,
    'home',
    '[data-testid="home-name"]',
    'data-controlled-value',
    name,
  );
  await waitForElementEnabled(app, 'home', '[data-testid="home-greet"]');
  await app.page.click({ page: 'home', css: '[data-testid="home-greet"]' });

  expect(await waitForElementText(
    app,
    'home',
    '[data-testid="home-greeting"]',
    (text) => text.includes(name),
    30_000,
  )).toContain(name);
});

spec('switches display language from the home control', async () => {
  const app = showcaseApp();
  await app.nav.relaunch({ page: 'home' });
  await app.page.waitFor({ page: 'home', css: '[data-testid="home-language"]' });

  const original = await app.eval({
    script: 'return lx.app.control.displayLanguage.getPreference()',
  }) as string;

  try {
    await app.page.click({ page: 'home', css: '[data-testid="home-language-zh-CN"]' });
    expect(await waitForElementText(
      app,
      'home',
      '[data-testid="home-tagline"]',
      (text) => text.includes('轻量应用框架'),
    )).toContain('轻量应用框架');
    await waitForElementAttribute(
      app,
      'home',
      '[data-testid="home-language-zh-CN"]',
      'data-selected',
      'true',
    );

    await app.page.click({ page: 'home', css: '[data-testid="home-language-en-US"]' });
    expect(await waitForElementText(
      app,
      'home',
      '[data-testid="home-tagline"]',
      (text) => text.includes('Lightweight Application Framework'),
    )).toContain('Lightweight Application Framework');
    await waitForElementAttribute(
      app,
      'home',
      '[data-testid="home-language-en-US"]',
      'data-selected',
      'true',
    );
  } finally {
    await app.eval({
      script: `await lx.app.control.displayLanguage.setPreference(${JSON.stringify(original)})`,
    });
  }
});
