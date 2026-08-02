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

  test('reads the host surface plan with JavaScript-shaped fields', async () => {
    const layout = await lx.automation().lxapp().surfaceLayout();
    const rootId = layout.mainSwitcher.rootSurfaceId;
    const root = layout.mainSwitcher.items.find((item) => item.surfaceId === rootId);

    expect(rootId).toBe('lingxia-showcase');
    expect(root?.root).toBeTruthy();
    if (root?.content.kind !== 'lxapp') {
      throw new Error(`expected an lxapp root, got ${root?.content.kind ?? 'missing'}`);
    }
    expect(root.content.appId).toBe('lingxia-showcase');
    const serialized = JSON.stringify(layout);
    expect(serialized.includes('"app_id"')).toBeFalsy();
    expect(serialized.includes('"surface_id"')).toBeFalsy();
    expect(serialized.includes('"active_id"')).toBeFalsy();
  });
});
