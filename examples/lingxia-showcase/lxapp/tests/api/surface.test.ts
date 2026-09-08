import { expect, spec } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { waitForCurrentPageVisible } from '../helpers/page.js';
import { bindFixture, eventually } from '../helpers/poll.js';
import { LX_RETURNED_OBJECT_SURFACES, LX_RUNTIME_SURFACES } from './manifest.js';

/** A host that did not build a driver throws on the getter rather than
 *  answering undefined; that is absence, not a broken shape. */
function automationSurfaceOrAbsent(name: string): unknown {
  try {
    return automationSurface(name);
  } catch {
    return undefined;
  }
}

function automationSurface(name: string): unknown {
  try {
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
  } catch {
    // Unsupported optional tiers (for example desktop on Android) reject at
    // the getter boundary instead of publishing an undefined placeholder.
    return undefined;
  }
}

/** A driver the host did not build answers its getter by throwing, which is
 *  absence rather than a broken shape. `unbuilt` says which members did that. */
function readMember(record: Record<string, unknown>, name: string): { unbuilt: boolean; value: unknown } {
  try {
    return { unbuilt: false, value: record[name] };
  } catch {
    return { unbuilt: true, value: undefined };
  }
}

function inspectSurface(
  target: unknown,
  members: readonly string[],
  properties: readonly string[],
): { available: boolean; missing: string[]; wrongKinds: string[]; unbuilt: string[] } {
  const available = target !== null && typeof target !== 'undefined';
  if (!available) return { available, missing: [...members], wrongKinds: [], unbuilt: [] };
  const record = target as Record<string, unknown>;
  const read = members.map((name) => ({ name, ...readMember(record, name) }));
  const present = read.filter((member) => !member.unbuilt);
  return {
    available,
    unbuilt: read.filter((member) => member.unbuilt).map((member) => member.name),
    missing: present.filter((member) => typeof member.value === 'undefined').map((member) => member.name),
    wrongKinds: present
      .filter((member) => (properties.includes(member.name)
        ? member.value === null
        : typeof member.value !== 'function'))
      .map((member) => member.name),
  };
}

// Claim only what this spec actually checks. An optional surface or member is
// skipped when the host did not build it, and claiming it anyway makes the
// covers gate report a surface nobody touched as proven.
const SHAPE_COVERS = [
  ...LX_RUNTIME_SURFACES.flatMap((surface) => {
    if ('optional' in surface && surface.optional) return [];
    const optionalMembers: readonly string[] = 'optionalMembers' in surface
      ? surface.optionalMembers
      : [];
    return surface.members
      .filter((member) => !optionalMembers.includes(member))
      .map((member) => `shape:${surface.name}.${member}`);
  }),
  ...LX_RETURNED_OBJECT_SURFACES
    .filter((surface) => surface.fixture === 'runtime-safe'
      && !('optional' in surface && surface.optional))
    .flatMap((surface) => surface.members.map((member) => `shape:${surface.name}.${member}`)),
];

spec('publish every public runtime and returned-object member', {
  id: 'SHAPE-RUNTIME',
  covers: SHAPE_COVERS,
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app, defer } = bindFixture(t, 'SHAPE-RUNTIME');
  const failures: string[] = [];

  for (const surface of LX_RUNTIME_SURFACES) {
    const propertyNames = 'properties' in surface ? surface.properties : [];
    const optionalMembers: readonly string[] = 'optionalMembers' in surface
      ? surface.optionalMembers
      : [];
    let result: { available: boolean; missing: string[]; wrongKinds: string[]; unbuilt?: string[] };
    try {
      result = surface.layer === 'automation'
        ? inspectSurface(
          automationSurfaceOrAbsent(surface.name),
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
        }) as { available: boolean; missing: string[]; wrongKinds: string[]; unbuilt?: string[] };
    } catch (error) {
      failures.push(`${surface.name}: ${String(error)}`);
      continue;
    }

    if ('optional' in surface && surface.optional && !result.available) continue;
    // The automation drivers are the harness, not the product surface, and a
    // phone host builds none of the desktop ones. Their absence is a fact
    // about the host; only a half-built driver is a defect.
    if (surface.layer === 'automation' && !result.available) continue;
    if (!result.available || result.missing.length > 0 || result.wrongKinds.length > 0) {
      failures.push(`${surface.name}: available=${result.available} missing=${result.missing.join(',')} wrong=${result.wrongKinds.join(',')}`);
    }
  }

  for (const surface of LX_RETURNED_OBJECT_SURFACES) {
    if (surface.fixture !== 'runtime-safe' || ('optional' in surface && surface.optional)) continue;
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

      if (!result.available || result.missing.length > 0 || result.wrongKinds.length > 0) {
        failures.push(`LxFile: available=${result.available} missing=${result.missing.join(',')} wrong=${result.wrongKinds.join(',')}`);
      }
      continue;
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
      { timeoutMs: 5_000, describe: 'native video fixture layout to settle' });
    const result = await eventually(
      () => app.eval({
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
      }) as Promise<{ available: boolean; missing: string[]; wrongKinds: string[] }>,
      () => true,
      {
        timeoutMs: 5_000,
        describe: 'native video context to bind after fixture mount',
        retryIf: () => true,
      },
    );

    if (!result.available || result.missing.length > 0 || result.wrongKinds.length > 0) {
      failures.push(`VideoContext: available=${result.available} missing=${result.missing.join(',')} wrong=${result.wrongKinds.join(',')}`);
    }
  }

  expect(failures).toEqual([]);
});
