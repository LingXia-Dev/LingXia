import { expect } from '@rongjs/test';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { waitForCurrentPageVisible } from '../helpers/page.js';
import { contract, eventually } from '../support/contract.js';
import { LX_RETURNED_OBJECT_SURFACES, LX_RUNTIME_SURFACES } from './manifest.js';

function automationSurface(name: string): unknown {
  const automation = lx.automation();
  switch (name) {
    case 'Automation': return automation;
    case 'ShellDriver': return automation.shell;
    case 'TerminalDriver': return automation.terminal;
    case 'LxAppDriver': return automation.lxapp(SHOWCASE_APP_ID);
    case 'PageDriver': return automation.lxapp(SHOWCASE_APP_ID).page;
    case 'PagePointer': return automation.lxapp(SHOWCASE_APP_ID).page.pointer;
    case 'PageKey': return automation.lxapp(SHOWCASE_APP_ID).page.key;
    case 'NavDriver': return automation.lxapp(SHOWCASE_APP_ID).nav;
    case 'LxAppManager': return automation.lxapps;
    case 'DeviceDriver': return automation.device;
    case 'BrowserDriver': return automation.browser;
    case 'BrowserCookies': return automation.browser.cookies;
    case 'DesktopDriver': return automation.desktop;
    case 'DesktopPointer': return automation.desktop.pointer;
    case 'DesktopKey': return automation.desktop.key;
    case 'DesktopWindow': return automation.desktop.window;
    case 'DesktopClipboard': return automation.desktop.clipboard;
    case 'DesktopAx': return automation.desktop.ax;
    case 'DesktopWait': return automation.desktop.wait;
    case 'DesktopApp': return automation.desktop.app;
    case 'DesktopProcess': return automation.desktop.process;
    default: throw new Error(`No host automation resolver for ${name}`);
  }
}

function inspectSurface(
  target: unknown,
  members: readonly string[],
  properties: readonly string[],
): { available: boolean; missing: string[]; wrongKinds: string[] } {
  const available = target !== null && typeof target !== 'undefined';
  const record = target as Record<string, unknown>;
  return {
    available,
    missing: available ? members.filter((name) => typeof record[name] === 'undefined') : [...members],
    wrongKinds: available
      ? members.filter((name) => properties.includes(name)
        ? record[name] === null
        : typeof record[name] !== 'function')
      : [],
  };
}

LX_RUNTIME_SURFACES.forEach((surface) => {
  contract({
    id: `SHAPE-${surface.name.replace(/[^a-zA-Z0-9]+/g, '-').toLocaleUpperCase()}`,
    title: `publish every ${surface.name} member`,
    covers: surface.members.map((member) => `shape:${surface.name}.${member}`),
    layer: surface.layer,
    levels: ['shape'],
    scope: 'portable',
    expectedOutcome: 'optional' in surface && surface.optional
      ? 'supported-or-absent'
      : 'supported',
  }, async ({ app }) => {
    const propertyNames = 'properties' in surface ? surface.properties : [];
    const optionalMembers: readonly string[] = 'optionalMembers' in surface
      ? surface.optionalMembers
      : [];
    const result = surface.layer === 'automation'
      ? inspectSurface(
        automationSurface(surface.name),
        surface.members.filter((name) => !optionalMembers.includes(name)),
        propertyNames,
      )
      : await app.eval({
        script: `
          const target = ${surface.expression};
          const members = ${JSON.stringify(surface.members)};
          const optionalMembers = ${JSON.stringify(optionalMembers)};
          const properties = ${JSON.stringify(propertyNames)};
          return {
            available: target !== null && typeof target !== 'undefined',
            missing: target == null
              ? members
              : members.filter((name) => (
                typeof target[name] === 'undefined' && !optionalMembers.includes(name)
              )),
            wrongKinds: target == null
              ? []
              : members.filter((name) => !optionalMembers.includes(name) && (
                properties.includes(name)
                  ? target[name] === null
                  : typeof target[name] !== 'function'
              )),
          };
        `,
      }) as { available: boolean; missing: string[]; wrongKinds: string[] };

    if ('optional' in surface && surface.optional && !result.available) return;
    expect(result.available).toBeTruthy();
    expect(result.missing).toEqual([]);
    expect(result.wrongKinds).toEqual([]);
  });
});

LX_RETURNED_OBJECT_SURFACES.forEach((surface) => {
  if (surface.fixture !== 'runtime-safe') return;

  contract({
    id: `SHAPE-${surface.name.replace(/[^a-zA-Z0-9]+/g, '-').toLocaleUpperCase()}`,
    title: `publish every ${surface.name} member on a real runtime instance`,
    covers: surface.members.map((member) => `shape:${surface.name}.${member}`),
    layer: 'logic',
    levels: ['shape'],
    scope: 'portable',
    expectedOutcome: 'supported',
  }, async ({ app, defer }) => {
    const fixtureName: string = surface.name;
    if (fixtureName === 'LxFile') {
      const result = await app.eval({
        script: `
          const target = lx.fs.file('lx://userdata/__shape__/managed-file');
          const members = ${JSON.stringify(surface.members)};
          const properties = ${JSON.stringify(surface.properties)};
          const optionalProperties = ${JSON.stringify(surface.optionalProperties)};
          return {
            available: target !== null && typeof target !== 'undefined',
            missing: target == null
              ? members
              : members.filter((name) => (
                typeof target[name] === 'undefined' && !optionalProperties.includes(name)
              )),
            wrongKinds: target == null
              ? []
              : members.filter((name) => properties.includes(name)
                ? target[name] === null
                  || (typeof target[name] === 'undefined' && !optionalProperties.includes(name))
                : typeof target[name] !== 'function'),
          };
        `,
      }) as { available: boolean; missing: string[]; wrongKinds: string[] };

      expect(result.available).toBeTruthy();
      expect(result.missing).toEqual([]);
      expect(result.wrongKinds).toEqual([]);
      return;
    }
    if (fixtureName !== 'VideoContext') {
      throw new Error(`No runtime-safe fixture resolver for ${fixtureName}`);
    }
    await app.nav.relaunch({
      page: 'video',
      query: { automationFixture: 'video-context-shape' },
    });
    defer(async () => {
      await app.nav.relaunch({ page: 'home' });
      await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
    });
    await app.page.waitFor({ page: 'video', css: '#lx-video-shape-fixture', state: 'attached' });
    let stableRectSamples = 0;
    let previousRect = '';
    await eventually(
      () => app.page.eval({
        page: 'video',
        script: `(() => {
          const rect = document.querySelector('#lx-video-shape-fixture')?.getBoundingClientRect();
          return rect && rect.width > 1 && rect.height > 1
            ? [rect.x, rect.y, rect.width, rect.height].map(Math.round).join(',')
            : '';
        })()`,
      }),
      (rect) => {
        if (typeof rect !== 'string' || rect.length === 0) {
          previousRect = '';
          stableRectSamples = 0;
          return false;
        }
        stableRectSamples = rect === previousRect ? stableRectSamples + 1 : 1;
        previousRect = rect;
        return stableRectSamples >= 2;
      },
      { timeoutMs: 5_000, describe: 'native video fixture layout to settle' },
    );
    const result = await app.eval({
      script: `
        const target = lx.createVideoContext('lx-video-shape-fixture');
        const members = ${JSON.stringify(surface.members)};
        const properties = ${JSON.stringify(surface.properties)};
        const optionalProperties = ${JSON.stringify(surface.optionalProperties)};
        return {
          available: target !== null && typeof target !== 'undefined',
          missing: target == null
            ? members
            : members.filter((name) => (
              typeof target[name] === 'undefined' && !optionalProperties.includes(name)
            )),
          wrongKinds: target == null
            ? []
            : members.filter((name) => properties.includes(name)
              ? target[name] === null
                || (typeof target[name] === 'undefined' && !optionalProperties.includes(name))
              : typeof target[name] !== 'function'),
        };
      `,
    }) as { available: boolean; missing: string[]; wrongKinds: string[] };

    expect(result.available).toBeTruthy();
    expect(result.missing).toEqual([]);
    expect(result.wrongKinds).toEqual([]);
  });
});
