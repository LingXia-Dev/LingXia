import { describe, expect, test } from '@rongjs/test';

describe('lx automation', () => {
  test('selects and inspects the current lxapp', async () => {
    const app = lx.automation().lxapp();
    const info = await app.info();
    const pages = await app.pages();

    expect(info.appid).toBe('lingxia-showcase');
    expect(pages.some((page) => page.name === 'todo')).toBeTruthy();
  });

  test('evaluates across the Logic boundary', async () => {
    const value = await lx.automation().lxapp().eval({ script: '21 * 2' });

    expect(value).toBe(42);
  });
});
